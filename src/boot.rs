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
