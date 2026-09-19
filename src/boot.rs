// @pinker-nav:start boot.generation.freestanding-boundary
// @pinker-nav:domain generation
// @pinker-nav:layer boot
// @pinker-nav:summary FREESTANDING_BOOT_ENTRY_FUNCTION ("principal") and FREESTANDING_BOOT_ENTRY_SYMBOL ("_start") are textual constants derived from the single authority `native_symbol` (not literals of their own); freestanding_linker_script returns the literal string of a GNU ld linker script with `. = 1M;` and the .text/.rodata/.data/.bss sections; freestanding_kernel_stub assembles via format! a two-instruction string (`call principal` followed by a `.Lpinker_hang` label with a `jmp` to itself, an infinite loop). The two functions and the two constants only produce boundary strings/constants — none of them executes, allocates, links, assembles or initializes hardware/stack/Multiboot/UEFI; `1M` and `principal` here are merely text embedded in the output, not values computed or checked against the rest of the pipeline.
pub const FREESTANDING_BOOT_ENTRY_FUNCTION: &str = crate::native_symbol::ENTRYPOINT_SOURCE_IDENTITY;
pub const FREESTANDING_BOOT_ENTRY_SYMBOL: &str =
    crate::native_symbol::FREESTANDING_ENTRYPOINT_SYMBOL;

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
// @pinker-nav:end boot.generation.freestanding-boundary
