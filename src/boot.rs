// @pinker-nav:start boot.geracao.fronteira-freestanding
// @pinker-nav:domain geracao
// @pinker-nav:layer boot
// @pinker-nav:summary FREESTANDING_BOOT_ENTRY_FUNCTION ("principal") e FREESTANDING_BOOT_ENTRY_SYMBOL ("_start") são constantes textuais; freestanding_linker_script retorna a string literal de um script de linker GNU ld com `. = 1M;` e as seções .text/.rodata/.data/.bss; freestanding_kernel_stub monta via format! uma string de duas instruções (`call principal` seguido de um rótulo `.Lpinker_hang` com `jmp` para si mesmo, um laço infinito). As três funções só produzem strings/constantes de fronteira — nenhuma delas executa, aloca, linka, monta ou inicializa hardware/stack/Multiboot/UEFI; `1M` e `principal` aqui são apenas texto embutido no output, não valores calculados ou verificados contra o restante do pipeline.
pub const FREESTANDING_BOOT_ENTRY_FUNCTION: &str = "principal";
pub const FREESTANDING_BOOT_ENTRY_SYMBOL: &str = "_start";

pub fn freestanding_linker_script() -> &'static str {
    "ENTRY(_start)\nSECTIONS\n{\n  . = 1M;\n  .text : { *(.text*) }\n  .rodata : { *(.rodata*) }\n  .data : { *(.data*) }\n  .bss : { *(.bss*) *(COMMON) }\n}"
}

pub fn freestanding_kernel_stub() -> String {
    format!(
        "{symbol}:\n  call {entry}\n.Lpinker_hang:\n  jmp .Lpinker_hang",
        symbol = FREESTANDING_BOOT_ENTRY_SYMBOL,
        entry = FREESTANDING_BOOT_ENTRY_FUNCTION,
    )
}
// @pinker-nav:end boot.geracao.fronteira-freestanding
