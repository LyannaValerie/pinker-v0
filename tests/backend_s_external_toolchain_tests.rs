mod common;

use common::render_backend_s_external_subset;
use pinker_v0::backend_s::emit_external_toolchain_subset;
use pinker_v0::cfg_ir::OperandIR;
use pinker_v0::instr_select::{
    SelectedBlock, SelectedFunction, SelectedGlobal, SelectedProgram, SelectedTerminator,
};
use pinker_v0::ir::TypeIR;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn asm_s_external_subset_emite_main_montavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(
        "# pinker v0 external toolchain subset (fase 129, linux x86_64, frame/reg + memoria minima + multiplos blocos/labels + jmp/br + loop minimo + quebrar/continuar camada 3 conservadora (composicao minima ate tres niveis de laço) + globais estaticas minimas em .rodata + abi minima mais larga ate 3 args + composto minimo com deref_store homogêneo e ninho heterogeneo camada 1 (`bombom`+`u32` em leitura por offset) + u32/u64 minimos em params/locals + comparacao `>=` minima (camada 4 conservadora de 10.2))"
    ));
    assert!(out.contains(".globl main"));
    assert!(out.contains("jmp .Lprincipal_entry"));
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("movabsq $42, %rax"));
}

#[test]
fn asm_s_external_subset_fase111_exemplo_versionado_emite_labels_e_jmp_incondicional() {
    let code = include_str!("../examples/fase111_blocos_labels_salto_incondicional_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("jmp .Lprincipal_entry"));
}

#[test]
fn asm_s_external_subset_fase112_exemplo_versionado_emite_cmp_e_jcc() {
    let code = include_str!("../examples/fase112_branch_condicional_minimo_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_entry:"));
    assert!(out.contains("cmpq %r10, %rax"));
    assert!(out.contains("cmpq $0, %rax"));
    assert!(out.contains("jne .Lprincipal_"));
}

#[test]
fn asm_s_external_subset_fase113_exemplo_versionado_emite_ciclo_com_label_de_loop() {
    let code = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_loop_cond_0:"));
    assert!(out.contains("setb %al"));
    assert!(out.contains("jmp .Lprincipal_loop_cond_0"));
}

#[test]
fn asm_s_external_subset_fase114_exemplo_versionado_emite_rodata_e_load_global() {
    let code = include_str!("../examples/fase114_globais_minimas_rodata_base_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".section .rodata"));
    assert!(out.contains(".globl BASE"));
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
    assert!(out.contains(".globl soma_par_minimo"));
    assert!(out.contains("movq (%rax), %rax"));
    assert!(out.contains("movabsq $8, %r10"));
}

#[test]
fn asm_s_external_subset_fase117_exemplo_versionado_emite_offset_explicito_em_local_ponteiro() {
    let code = include_str!("../examples/fase117_compostos_minimos_camada2_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl soma_par_offset_local"));
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.matches("movq (%rax), %rax").count() >= 2);
}

#[test]
fn asm_s_external_subset_fase118_exemplo_versionado_emite_deref_store_minimo() {
    let code = include_str!("../examples/fase118_compostos_minimos_camada3_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl soma_tripla_com_store_minimo"));
    assert!(out.contains("movq %r10, (%rax)"));
    assert!(out.matches("movq (%rax), %rax").count() >= 3);
}

#[test]
fn asm_s_external_subset_fase119_exemplo_versionado_consolida_par_homogeneo_minimo() {
    let code = include_str!("../examples/fase119_compostos_minimos_camada4_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl consolida_par_homogeneo_minimo"));
    assert!(out.matches("movq %r10, (%rax)").count() >= 2);
    assert!(out.matches("movq (%rax), %rax").count() >= 4);
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("movabsq $16, %r10"));
}

#[test]
fn asm_s_external_subset_fase120_exemplo_versionado_u32_minimo_em_param_local() {
    let code = include_str!("../examples/fase120_tipos_inteiros_mais_largos_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl eco_u32_minimo"));
    assert!(out.contains("call eco_u32_minimo"));
    assert!(out.contains("movq %rdi, -8(%rbp)"));
}

#[test]
fn asm_s_external_subset_fase121_exemplo_versionado_u64_minimo_em_param_local() {
    let code = include_str!("../examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl eco_u64_minimo"));
    assert!(out.contains("call eco_u64_minimo"));
    assert!(out.contains("movq %rdi, -8(%rbp)"));
}
#[test]
fn asm_s_external_subset_fase122_exemplo_versionado_comparacao_ne_minima() {
    let code = include_str!("../examples/fase122_comparacoes_ampliadas_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl diferente_u64_minimo"));
    assert!(out.contains("setne %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase123_exemplo_versionado_comparacao_gt_minima() {
    let code = include_str!("../examples/fase123_comparacoes_ampliadas_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl maior_u64_minimo"));
    assert!(out.contains("seta %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase124_exemplo_versionado_comparacao_le_minima() {
    let code = include_str!("../examples/fase124_comparacoes_ampliadas_camada3_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl menor_ou_igual_u64_minimo"));
    assert!(out.contains("setbe %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase125_exemplo_versionado_comparacao_ge_minima() {
    let code = include_str!("../examples/fase125_comparacoes_ampliadas_camada4_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl maior_ou_igual_u64_minimo"));
    assert!(out.contains("setae %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase129_exemplo_versionado_ninho_heterogeneo_camada1() {
    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".globl ler_marca_heterogenea_minima"));
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("sete %al"));
    assert!(out.contains("movabsq $129, %rax"));
}

#[test]
fn asm_s_external_subset_fase129_recusa_campo_heterogeneo_fora_recorte() {
    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_invalido.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Fase 129"));
    assert!(msg.contains("deref_load") || msg.contains("slot") || msg.contains("seta"));
}

#[test]
fn asm_s_external_subset_fase128_exemplo_versionado_quebrar_continuar_camada3() {
    let code = include_str!("../examples/fase128_quebrar_continuar_camada3_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_loop_cond_0:"));
    assert!(out.contains(".Lprincipal_loop_cond_2:"));
    assert!(out.contains(".Lprincipal_loop_cond_6:"));
    assert!(out.matches(".Lprincipal_loop_break_cont_").count() >= 3);
    assert!(out.matches(".Lprincipal_loop_continue_cont_").count() >= 3);
}

#[test]
fn asm_s_external_subset_fase127_exemplo_versionado_quebrar_continuar_camada2() {
    let code = include_str!("../examples/fase127_quebrar_continuar_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_loop_cond_0:"));
    assert!(out.contains(".Lprincipal_loop_cond_2:"));
    assert!(out.contains(".Lprincipal_loop_break_cont_"));
    assert!(out.contains(".Lprincipal_loop_continue_cont_"));
    assert!(out.matches("jmp .Lprincipal_loop_cond_").count() >= 2);
}

#[test]
fn asm_s_external_subset_fase126_exemplo_versionado_quebrar_continuar_camada1() {
    let code = include_str!("../examples/fase126_quebrar_continuar_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lprincipal_loop_cond_0:"));
    assert!(out.contains(".Lprincipal_loop_break_cont_"));
    assert!(out.contains(".Lprincipal_loop_continue_cont_"));
    assert!(out.contains("jmp .Lprincipal_loop_cond_0"));
}

#[test]
fn asm_s_external_subset_fluxo_real_fase117_composto_minimo_camada2() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase117_compostos_minimos_camada2_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl soma_par_offset_local"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase118_compostos_minimos_camada3_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl soma_tripla_com_store_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase119_compostos_minimos_camada4_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl consolida_par_homogeneo_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase120_tipos_inteiros_mais_largos_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl eco_u32_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl eco_u64_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase122_comparacoes_ampliadas_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl diferente_u64_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase123_comparacoes_ampliadas_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl maior_u64_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase124_comparacoes_ampliadas_camada3_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl menor_ou_igual_u64_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase125_comparacoes_ampliadas_camada4_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl maior_ou_igual_u64_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl ler_marca_heterogenea_minima"));
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
fn asm_s_external_subset_fluxo_real_fase128_quebrar_continuar_camada3() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase128_quebrar_continuar_camada3_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lprincipal_loop_cond_0:"));
    assert!(asm.contains(".Lprincipal_loop_cond_2:"));
    assert!(asm.contains(".Lprincipal_loop_cond_6:"));
    assert!(asm.matches(".Lprincipal_loop_break_cont_").count() >= 3);
    assert!(asm.matches(".Lprincipal_loop_continue_cont_").count() >= 3);

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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase127_quebrar_continuar_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lprincipal_loop_cond_0:"));
    assert!(asm.contains(".Lprincipal_loop_cond_2:"));
    assert!(asm.contains(".Lprincipal_loop_break_cont_"));
    assert!(asm.contains(".Lprincipal_loop_continue_cont_"));

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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase126_quebrar_continuar_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lprincipal_loop_break_cont_"));
    assert!(asm.contains(".Lprincipal_loop_continue_cont_"));

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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase116_compostos_minimos_camada1_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl soma_par_minimo"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lprincipal_loop_cond_0:"));
    assert!(asm.contains("jmp .Lprincipal_loop_cond_0"));
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

#[test]
fn asm_s_external_subset_fluxo_real_com_locais_e_aritmetica() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
        return;
    };

    let code = include_str!("../examples/fase74_backend_externo_call_minimo_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl main"));
    assert!(asm.contains(".globl dobro"));
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }

    let Some(driver) = detect_cc_driver() else {
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

#[test]
fn asm_s_external_subset_falha_clara_fora_do_subset() {
    let code =
        include_str!("../examples/fase115_abi_minima_mais_larga_camada1_quatro_args_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 115)"));
}

#[test]
fn asm_s_external_subset_falha_parametro_nao_bombom() {
    let code =
        include_str!("../examples/fase75_backend_externo_parametro_nao_bombom_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 129) aceita parâmetro `bombom`, `u32`, `u64` ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase115_preserva_recusa_explicita_quatro_parametros_por_funcao() {
    let code =
        include_str!("../examples/fase115_abi_minima_mais_larga_camada1_quatro_args_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 115) recusa explicitamente 4+ parâmetros por função",
    ));
}

#[test]
fn asm_s_external_subset_fase112_aceita_talvez_senao_no_recorte_minimo() {
    let code = include_str!(
        "../examples/fase82_backend_externo_recusa_explicita_talvez_senao_invalido.pink",
    );

    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("cmpq $0, %rax"));
    assert!(asm.contains("jne .Lprincipal_"));
}

#[test]
fn asm_s_external_subset_fase113_recusa_loop_com_condicao_fora_do_recorte() {
    let code = include_str!("../examples/fase113_loop_condicao_invalida_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 129) aceita apenas atribuição, aritmética linear (+,-,*), comparações mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), call direta com até 3 argumentos (`bombom`/`u32`/`u64`/`seta<T>`), `deref_store` homogêneo em `seta<bombom>`, `deref_load` mínimo em `bombom`/`u32` (incluindo campo heterogêneo de `ninho` via offset explícito), load/store em slots de frame e recorte conservador de `quebrar`/`continuar` em `sempre que` via saltos já materializados (com composição mínima auditável até três níveis de laço aninhado)"
    ));
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
        assert!(asm.contains("# pinker v0 external toolchain subset (fase 129"));
    }

    let caso_rejeitado_tres_params = include_str!(
        "../examples/fase115_abi_minima_mais_larga_camada1_quatro_args_invalido.pink",
    );
    let err_tres_params = render_backend_s_external_subset(caso_rejeitado_tres_params).unwrap_err();
    assert!(err_tres_params.to_string().contains(
        "subset externo montável (Fase 115) recusa explicitamente 4+ parâmetros por função"
    ));

    let caso_branch_valido =
        include_str!("../examples/fase112_branch_condicional_minimo_valido.pink");
    let asm_branch = render_backend_s_external_subset(caso_branch_valido).unwrap();
    assert!(asm_branch.contains("cmpq $0, %rax"));
    assert!(asm_branch.contains("jne .Lprincipal_"));

    let caso_loop_valido = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let asm_loop = render_backend_s_external_subset(caso_loop_valido).unwrap();
    assert!(asm_loop.contains(".Lprincipal_loop_cond_0:"));
    assert!(asm_loop.contains("jmp .Lprincipal_loop_cond_0"));

    let caso_rejeitado_sempre_que =
        include_str!("../examples/fase113_loop_condicao_invalida_invalido.pink");
    let err_sempre_que = render_backend_s_external_subset(caso_rejeitado_sempre_que).unwrap_err();
    assert!(err_sempre_que.to_string().contains(
        "subset externo montável (Fase 129) aceita apenas atribuição, aritmética linear (+,-,*), comparações mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), call direta com até 3 argumentos (`bombom`/`u32`/`u64`/`seta<T>`), `deref_store` homogêneo em `seta<bombom>`, `deref_load` mínimo em `bombom`/`u32` (incluindo campo heterogêneo de `ninho` via offset explícito), load/store em slots de frame e recorte conservador de `quebrar`/`continuar` em `sempre que` via saltos já materializados (com composição mínima auditável até três níveis de laço aninhado)"
    ));
}

#[test]
fn asm_s_external_subset_fase116_recusa_composto_fora_da_camada1() {
    let code = include_str!("../examples/fase116_compostos_minimos_camada1_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 129) aceita parâmetro `bombom`, `u32`, `u64` ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase117_recusa_local_composto_fora_da_camada2() {
    let code = include_str!("../examples/fase117_compostos_minimos_camada2_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 129) só aceita local `bombom`, `u32`, `u64` ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase118_recusa_store_fragil_fora_do_subset() {
    let code = include_str!("../examples/fase118_compostos_minimos_camada3_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 129) aceita parâmetro `bombom`, `u32`, `u64` ou `seta<T>`"
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
        "subset externo montável (Fase 129) aceita parâmetro `bombom`, `u32`, `u64` ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase114_falha_em_global_duplicada() {
    let program = SelectedProgram {
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
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
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
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
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
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
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
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
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

fn unique_temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema inválido")
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_phase113_{}", nanos))
}
