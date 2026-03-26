use crate::backend_text;
use crate::backend_text::BackendTextProgram;
use crate::boot::{
    freestanding_kernel_stub, freestanding_linker_script, FREESTANDING_BOOT_ENTRY_FUNCTION,
    FREESTANDING_BOOT_ENTRY_SYMBOL,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use crate::token::{Position, Span};
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn emit_from_selected(selected: &SelectedProgram) -> Result<String, PinkerError> {
    validate_supported_subset(selected)?;
    let lowered = backend_text::lower_selected_program(selected)?;
    Ok(render_program(&lowered))
}

/// Emite um `.s` mínimo montável por toolchain externa (assembler+linker do sistema).
///
/// Escopo deliberadamente mínimo para a Fase 130:
/// - target assumido: Linux x86_64 (SysV) hospedado;
/// - subset aceito: funções `-> bombom` com múltiplos blocos/labels, `jmp` incondicional, branch condicional mínimo e loop mínimo por retorno de salto entre blocos;
/// - disciplina mínima de registradores/frame: `%rax` (retorno/acumulador), `%rdi` (arg0), `%rsi` (arg1), `%rdx` (arg2), `%r10` (temporário volátil), slots em frame `%rbp`;
/// - memória mínima real garantida: load/store em slots de frame via `movq -off(%rbp), %reg` e `movq %reg, -off(%rbp)`;
/// - branch condicional mínimo via teste contra zero (`cmpq $0` + `jne`) e sem ABI completa.
/// - globais estáticas mínimas somente-leitura em `.rodata`: `eterno` de valor literal inteiro/lógico com leitura por símbolo `@nome(%rip)`.
/// - composto mínimo conservador: base homogênea `seta<bombom>` com `deref_store` mínimo e abertura heterogênea em duas camadas para `ninho` apenas em leitura auditável de campo escalar `u32`/`u64` via `deref_load` + offset explícito;
/// - inteiros fixos adicionais no recorte externo: `u32` (Fase 120) e `u64` (Fase 121) em parâmetros e locais, reaproveitando movimentação/call no mesmo frame/ABI mínima existente;
/// - `quebrar`/`continuar` (Fase 128, camada 3 conservadora) no recorte de `sempre que` já materializado em `selected`, com composição mínima auditável de três níveis de laço (`sempre que` externo/meio/interno) sem abrir subsistema geral de controle de fluxo.
///
/// O resultado mapeia `principal` para o símbolo `main`, para permitir linkedição
/// via driver C (`cc`/`gcc`/`clang`) sem runtime próprio.
pub fn emit_external_toolchain_subset(selected: &SelectedProgram) -> Result<String, PinkerError> {
    let program = extract_external_callconv_program(selected)?;
    Ok(render_external_x86_64_linux_callconv(&program))
}

fn validate_supported_subset(selected: &SelectedProgram) -> Result<(), PinkerError> {
    for function in &selected.functions {
        if !is_supported_type(function.ret_type) {
            return Err(err(&format!(
                "backend .s textual ainda não suporta retorno '{}' em '{}'",
                function.ret_type.name(),
                function.name
            )));
        }

        for (slot, ty) in &function.slot_types {
            if !is_supported_type(*ty) {
                return Err(err(&format!(
                    "backend .s textual ainda não suporta slot '{}' do tipo '{}' em '{}'",
                    slot,
                    ty.name(),
                    function.name
                )));
            }
        }

        for block in &function.blocks {
            for inst in &block.instructions {
                if let SelectedInstr::Call { ret_type, .. } = inst {
                    if !is_supported_type(*ret_type) {
                        return Err(err(&format!(
                            "backend .s textual ainda não suporta call com retorno '{}'",
                            ret_type.name()
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

struct ExternalCallConvProgram {
    rodata_globals: Vec<ExternalCallConvGlobal>,
    functions: Vec<ExternalCallConvFunction>,
}

struct ExternalCallConvGlobal {
    name: String,
    value: u64,
}

struct ExternalCallConvFunction {
    name: String,
    stack_size: u32,
    slot_offsets: HashMap<String, u32>,
    blocks: Vec<ExternalCallConvBlock>,
    params: Vec<String>,
}

struct ExternalCallConvBlock {
    label: String,
    body: Vec<String>,
    terminator: ExternalCallConvTerminator,
}

enum ExternalCallConvTerminator {
    Jmp(String),
    Br {
        cond: OperandIR,
        then_label: String,
        else_label: String,
    },
    Ret(OperandIR),
}

const REG_RET: &str = "%rax";
const ARG_REGS: [&str; 3] = ["%rdi", "%rsi", "%rdx"];
const REG_TMP: &str = "%r10";

fn extract_external_callconv_program(
    selected: &SelectedProgram,
) -> Result<ExternalCallConvProgram, PinkerError> {
    let mut seen_globals = HashSet::new();
    let mut rodata_globals = Vec::new();
    for global in &selected.globals {
        if !seen_globals.insert(global.name.clone()) {
            return Err(err(
                "subset externo montável (Fase 114) encontrou símbolo global duplicado",
            ));
        }
        if global.ty != TypeIR::Bombom && global.ty != TypeIR::Logica {
            return Err(err(
                "subset externo montável (Fase 114) aceita apenas globais estáticas `bombom`/`logica`",
            ));
        }
        let value = match &global.value {
            OperandIR::Int(v) => *v,
            OperandIR::Bool(v) => u64::from(*v),
            _ => {
                return Err(err(
                    "subset externo montável (Fase 114) aceita apenas inicialização literal inteira/lógica em globais estáticas",
                ));
            }
        };
        rodata_globals.push(ExternalCallConvGlobal {
            name: global.name.clone(),
            value,
        });
    }

    let has_main = selected.functions.iter().any(|f| f.name == "principal");
    if !has_main {
        return Err(err(
            "subset externo montável (Fase 84) exige função `principal`",
        ));
    }

    let mut functions = Vec::new();
    for function in &selected.functions {
        if function.ret_type != TypeIR::Bombom {
            return Err(err(
                "subset externo montável (Fase 84) exige retorno `bombom` em todas as funções",
            ));
        }
        if function.name == "principal" && !function.params.is_empty() {
            return Err(err(
                "subset externo montável (Fase 84) exige `principal()` sem parâmetros",
            ));
        }
        if function.params.len() > ARG_REGS.len() {
            return Err(err(
                "subset externo montável (Fase 115) recusa explicitamente 4+ parâmetros por função; limite garantido: até 3 parâmetros `bombom`",
            ));
        }
        for param in &function.params {
            let Some(ty) = function.slot_types.get(param) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou parâmetro sem tipo",
                ));
            };
            if !is_external_param_type(ty) {
                return Err(err(
                "subset externo montável (Fase 130) aceita parâmetro `bombom`, `u32`, `u64` ou `seta<T>` no recorte conservador (inteiros mais largos + composto mínimo homogêneo/heterogêneo camada 1 + `quebrar`/`continuar` em loop mínimo)",
                ));
            }
        }
        for local in &function.locals {
            let Some(ty) = function.slot_types.get(local) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou local sem tipo",
                ));
            };
            if !is_external_local_type(ty) {
                return Err(err(&format!(
                    "subset externo montável (Fase 130) só aceita local `bombom`, `u32`, `u64` ou `seta<T>`; '{}' é '{}'",
                    local,
                    ty.name()
                )));
            }
        }
        if function.blocks.is_empty() {
            return Err(err(
                "subset externo montável (Fase 111) exige ao menos um bloco por função",
            ));
        }
        validate_external_block_labels(function)?;

        let temp_ids = collect_temp_ids(function);
        let mut slot_offsets = HashMap::new();
        let mut slot_index = 1u32;
        for param in &function.params {
            slot_offsets.insert(param.clone(), slot_index * 8);
            slot_index += 1;
        }
        for local in &function.locals {
            slot_offsets.insert(local.clone(), slot_index * 8);
            slot_index += 1;
        }
        for temp in temp_ids {
            slot_offsets.insert(temp, slot_index * 8);
            slot_index += 1;
        }
        let raw_stack = (slot_index.saturating_sub(1)) * 8;
        let stack_size = if raw_stack == 0 {
            0
        } else {
            raw_stack.div_ceil(16) * 16
        };

        let mut blocks = Vec::new();
        for block in &function.blocks {
            let terminator = match &block.terminator {
                SelectedTerminator::Jmp(target) => ExternalCallConvTerminator::Jmp(target.clone()),
                SelectedTerminator::Ret(Some(value)) => {
                    ExternalCallConvTerminator::Ret(value.clone())
                }
                SelectedTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => ExternalCallConvTerminator::Br {
                    cond: cond.clone(),
                    then_label: then_label.clone(),
                    else_label: else_label.clone(),
                },
                _ => {
                    return Err(err(
                        "subset externo montável (Fase 113) exige terminador `jmp`, `br` ou `ret <valor>` em cada bloco",
                    ));
                }
            };
            let mut body = Vec::new();
            for inst in &block.instructions {
                match inst {
                    SelectedInstr::Mov { dest, src } => {
                        ensure_dest_is_local_or_param(dest, function)?;
                        body.extend(load_operand(REG_RET, src, &slot_offsets)?);
                        body.push(format!("movq {}, -{}(%rbp)", REG_RET, slot_offsets[dest]));
                    }
                    SelectedInstr::Add { dest, lhs, rhs } => {
                        body.extend(lower_linear_binop("addq", *dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::Sub { dest, lhs, rhs } => {
                        body.extend(lower_linear_binop("subq", *dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::Mul { dest, lhs, rhs } => {
                        body.extend(lower_linear_binop("imulq", *dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpEq { dest, lhs, rhs } => {
                        body.extend(lower_cmp_eq(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpNe { dest, lhs, rhs } => {
                        body.extend(lower_cmp_ne(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpLt { dest, lhs, rhs } => {
                        body.extend(lower_cmp_lt(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpGt { dest, lhs, rhs } => {
                        body.extend(lower_cmp_gt(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpLe { dest, lhs, rhs } => {
                        body.extend(lower_cmp_le(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::CmpGe { dest, lhs, rhs } => {
                        body.extend(lower_cmp_ge(*dest, lhs, rhs, &slot_offsets)?);
                    }
                    SelectedInstr::DerefLoad {
                        dest,
                        ptr,
                        ty,
                        is_volatile,
                    } => {
                        if !is_external_deref_load_type(ty) {
                            return Err(err(
                                "subset externo montável (Fase 130) aceita `deref_load` apenas no recorte mínimo `bombom`/`u32`/`u64` (camada 2 conservadora de `ninho` heterogêneo + legado homogêneo)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 130) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets)?);
                        body.push(format!("movq ({}), {}", REG_RET, REG_RET));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::DerefStore {
                        ptr,
                        value,
                        ty,
                        is_volatile,
                    } => {
                        if *ty != TypeIR::Bombom {
                            return Err(err(
                                "subset externo montável (Fase 130) aceita `deref_store` apenas para `seta<bombom>` (recorte homogêneo conservador preservado)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 130) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets)?);
                        body.extend(load_operand(REG_TMP, value, &slot_offsets)?);
                        body.push(format!("movq {}, ({})", REG_TMP, REG_RET));
                    }
                    SelectedInstr::Call {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        if *ret_type != TypeIR::Bombom {
                            return Err(err(
                                "subset externo montável (Fase 84) só aceita call com retorno `bombom`",
                            ));
                        }
                        if !selected.functions.iter().any(|f| &f.name == callee) {
                            return Err(err(
                                "subset externo montável (Fase 84) encontrou call para função inexistente",
                            ));
                        }
                        if callee == &function.name {
                            return Err(err(
                                "subset externo montável (Fase 84) não suporta recursão externa",
                            ));
                        }
                        if args.len() > ARG_REGS.len() {
                            return Err(err(
                                "subset externo montável (Fase 115) recusa explicitamente call com 4+ argumentos; limite garantido: até 3 argumentos `bombom`",
                            ));
                        }
                        for (idx, arg) in args.iter().enumerate() {
                            body.extend(load_operand(ARG_REGS[idx], arg, &slot_offsets)?);
                        }
                        body.push(format!("call {}", callee));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    _ => {
                        return Err(err(
                            "subset externo montável (Fase 130) aceita apenas atribuição, aritmética linear (+,-,*), comparações mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), call direta com até 3 argumentos (`bombom`/`u32`/`u64`/`seta<T>`), `deref_store` homogêneo em `seta<bombom>`, `deref_load` mínimo em `bombom`/`u32`/`u64` (incluindo campo heterogêneo de `ninho` via offset explícito), load/store em slots de frame e recorte conservador de `quebrar`/`continuar` em `sempre que` via saltos já materializados (com composição mínima auditável até três níveis de laço aninhado)",
                        ));
                    }
                }
            }
            blocks.push(ExternalCallConvBlock {
                label: block.label.clone(),
                body,
                terminator,
            });
        }

        functions.push(ExternalCallConvFunction {
            name: function.name.clone(),
            stack_size,
            slot_offsets,
            blocks,
            params: function.params.clone(),
        });
    }

    Ok(ExternalCallConvProgram {
        rodata_globals,
        functions,
    })
}

fn render_external_x86_64_linux_callconv(program: &ExternalCallConvProgram) -> String {
    let mut out = String::new();
    line(
        &mut out,
        0,
        "# pinker v0 external toolchain subset (fase 130, linux x86_64, frame/reg + memoria minima + multiplos blocos/labels + jmp/br + loop minimo + quebrar/continuar camada 3 conservadora (composicao minima ate tres niveis de laço) + globais estaticas minimas em .rodata + abi minima mais larga ate 3 args + composto minimo com deref_store homogêneo e ninho heterogeneo camada 2 (`bombom`+`u32`+`u64` em leitura por offset) + u32/u64 minimos em params/locals + comparacao `>=` minima (camada 4 conservadora de 10.2))",
    );
    if !program.rodata_globals.is_empty() {
        line(&mut out, 0, ".section .rodata");
        for global in &program.rodata_globals {
            line(&mut out, 0, &format!(".globl {}", global.name));
            line(&mut out, 0, &format!("{}:", global.name));
            line(&mut out, 1, &format!(".quad {}", global.value));
        }
    }
    line(&mut out, 0, ".text");

    for function in &program.functions {
        let symbol = if function.name == "principal" {
            "main".to_string()
        } else {
            function.name.clone()
        };
        line(&mut out, 0, &format!(".globl {}", symbol));
        line(&mut out, 0, &format!("{}:", symbol));
        line(&mut out, 1, "pushq %rbp");
        line(&mut out, 1, "movq %rsp, %rbp");
        if function.stack_size > 0 {
            line(&mut out, 1, &format!("subq ${}, %rsp", function.stack_size));
        }
        for (idx, param) in function.params.iter().enumerate() {
            line(
                &mut out,
                1,
                &format!(
                    "movq {}, -{}(%rbp)",
                    ARG_REGS[idx], function.slot_offsets[param]
                ),
            );
        }
        if function.stack_size > 0 {
            line(
                &mut out,
                1,
                &format!(
                    "# frame: %rbp base + {} bytes de slots",
                    function.stack_size
                ),
            );
        }
        line(&mut out, 1, &format!("jmp .L{}_entry", function.name));
        for block in &function.blocks {
            line(
                &mut out,
                0,
                &format!(".L{}_{}:", function.name, block.label),
            );
            for stmt in &block.body {
                line(&mut out, 1, stmt);
            }
            match &block.terminator {
                ExternalCallConvTerminator::Jmp(target) => {
                    line(&mut out, 1, &format!("jmp .L{}_{}", function.name, target));
                }
                ExternalCallConvTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => {
                    for stmt in load_operand(REG_RET, cond, &function.slot_offsets)
                        .expect("condição do branch deve ser carregável")
                    {
                        line(&mut out, 1, &stmt);
                    }
                    line(&mut out, 1, "cmpq $0, %rax");
                    line(
                        &mut out,
                        1,
                        &format!("jne .L{}_{}", function.name, then_label),
                    );
                    line(
                        &mut out,
                        1,
                        &format!("jmp .L{}_{}", function.name, else_label),
                    );
                }
                ExternalCallConvTerminator::Ret(value) => {
                    for stmt in load_operand(REG_RET, value, &function.slot_offsets)
                        .expect("retorno deve ser carregável")
                    {
                        line(&mut out, 1, &stmt);
                    }
                    line(&mut out, 1, "leave");
                    line(&mut out, 1, "ret");
                }
            }
        }
    }
    out
}

fn ensure_dest_is_local_or_param(
    dest: &str,
    function: &crate::instr_select::SelectedFunction,
) -> Result<(), PinkerError> {
    if function.locals.iter().any(|local| local == dest)
        || function.params.iter().any(|param| param == dest)
    {
        Ok(())
    } else {
        Err(err(
            "subset externo montável (Fase 84) só aceita escrita em parâmetros ou variáveis locais declaradas",
        ))
    }
}

fn lower_linear_binop(
    opcode: &str,
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("{} {}, {}", opcode, REG_TMP, REG_RET));
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_eq(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("sete %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_ne(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("setne %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_lt(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("setb %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_gt(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("seta %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_le(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("setbe %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn lower_cmp_ge(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push("setae %al".to_string());
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn collect_temp_ids(function: &crate::instr_select::SelectedFunction) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            match inst {
                SelectedInstr::Neg { dest, .. }
                | SelectedInstr::Not { dest, .. }
                | SelectedInstr::BitNot { dest, .. }
                | SelectedInstr::DerefLoad { dest, .. }
                | SelectedInstr::Cast { dest, .. }
                | SelectedInstr::BitAnd { dest, .. }
                | SelectedInstr::BitOr { dest, .. }
                | SelectedInstr::BitXor { dest, .. }
                | SelectedInstr::Shl { dest, .. }
                | SelectedInstr::Shr { dest, .. }
                | SelectedInstr::Add { dest, .. }
                | SelectedInstr::Sub { dest, .. }
                | SelectedInstr::Mul { dest, .. }
                | SelectedInstr::Div { dest, .. }
                | SelectedInstr::Mod { dest, .. }
                | SelectedInstr::CmpEq { dest, .. }
                | SelectedInstr::CmpNe { dest, .. }
                | SelectedInstr::CmpLt { dest, .. }
                | SelectedInstr::CmpLe { dest, .. }
                | SelectedInstr::CmpGt { dest, .. }
                | SelectedInstr::CmpGe { dest, .. }
                | SelectedInstr::Call { dest, .. } => {
                    ids.insert(temp_key(*dest));
                }
                _ => {}
            }
        }
    }
    for block in &function.blocks {
        if let SelectedTerminator::Ret(Some(OperandIR::Temp(temp))) = &block.terminator {
            ids.insert(temp_key(*temp));
        }
    }
    ids
}

fn load_operand(
    reg: &str,
    operand: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut lines = Vec::new();
    match operand {
        OperandIR::Int(v) => lines.push(format!("movabsq ${}, {}", v, reg)),
        OperandIR::Bool(v) => lines.push(format!("movabsq ${}, {}", if *v { 1 } else { 0 }, reg)),
        OperandIR::Local(slot) => {
            let Some(offset) = slot_offsets.get(slot) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou slot sem offset",
                ));
            };
            lines.push(format!("movq -{}(%rbp), {}", offset, reg));
        }
        OperandIR::Temp(temp) => {
            let key = temp_key(*temp);
            let Some(offset) = slot_offsets.get(&key) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou temporário sem offset",
                ));
            };
            lines.push(format!("movq -{}(%rbp), {}", offset, reg));
        }
        OperandIR::GlobalConst(name) => {
            lines.push(format!("movq {}(%rip), {}", name, reg));
        }
        _ => {
            return Err(err(
                "subset externo montável (Fase 114) só aceita operandos inteiros, locais, temporários e global estática literal",
            ));
        }
    }
    Ok(lines)
}

fn temp_key(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn validate_external_block_labels(
    function: &crate::instr_select::SelectedFunction,
) -> Result<(), PinkerError> {
    let mut labels = HashSet::new();
    for block in &function.blocks {
        if block.label.trim().is_empty() {
            return Err(err(
                "subset externo montável (Fase 113) encontrou bloco sem label",
            ));
        }
        if !labels.insert(block.label.clone()) {
            return Err(err(
                "subset externo montável (Fase 113) encontrou label duplicado em função",
            ));
        }
    }
    if !labels.contains("entry") {
        return Err(err(
            "subset externo montável (Fase 113) exige bloco `entry` em cada função",
        ));
    }
    for block in &function.blocks {
        match &block.terminator {
            SelectedTerminator::Jmp(target) => {
                if !labels.contains(target) {
                    return Err(err(
                        "subset externo montável (Fase 113) encontrou `jmp` para label inexistente",
                    ));
                }
            }
            SelectedTerminator::Br {
                cond,
                then_label,
                else_label,
            } => {
                if !matches!(
                    cond,
                    OperandIR::Int(_)
                        | OperandIR::Bool(_)
                        | OperandIR::Local(_)
                        | OperandIR::Temp(_)
                ) {
                    return Err(err(
                        "subset externo montável (Fase 113) exige condição de `br` em inteiro local/temporário/imediato",
                    ));
                }
                if !labels.contains(then_label) {
                    return Err(err(
                        "subset externo montável (Fase 113) encontrou `br` com alvo verdadeiro inexistente",
                    ));
                }
                if !labels.contains(else_label) {
                    return Err(err(
                        "subset externo montável (Fase 113) encontrou `br` com alvo falso inexistente",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_supported_type(ty: TypeIR) -> bool {
    matches!(
        ty,
        TypeIR::Bombom
            | TypeIR::U8
            | TypeIR::U16
            | TypeIR::U32
            | TypeIR::U64
            | TypeIR::I8
            | TypeIR::I16
            | TypeIR::I32
            | TypeIR::I64
            | TypeIR::Logica
            | TypeIR::Nulo
    )
}

fn is_external_deref_load_type(ty: &TypeIR) -> bool {
    *ty == TypeIR::Bombom || *ty == TypeIR::U32 || *ty == TypeIR::U64
}

fn is_external_param_type(ty: &TypeIR) -> bool {
    *ty == TypeIR::Bombom
        || *ty == TypeIR::U32
        || *ty == TypeIR::U64
        || *ty == TypeIR::Pointer { is_volatile: false }
}

fn is_external_local_type(ty: &TypeIR) -> bool {
    is_external_param_type(ty)
}

pub fn render_program(program: &BackendTextProgram) -> String {
    let mut out = String::new();

    line(
        &mut out,
        0,
        "; pinker v0 textual .s (fase 54, abi textual minima, derivado de --selected)",
    );
    line(&mut out, 0, &format!("; module {}", program.module_name));
    line(
        &mut out,
        0,
        &format!(
            "; mode {}",
            if program.is_freestanding {
                "livre (freestanding intent)"
            } else {
                "hospedado"
            }
        ),
    );
    line(&mut out, 0, "; abi pinker.text.v0");
    if program.is_freestanding {
        line(
            &mut out,
            0,
            &format!(
                "; boot.entry {} -> {}",
                FREESTANDING_BOOT_ENTRY_FUNCTION, FREESTANDING_BOOT_ENTRY_SYMBOL
            ),
        );
        line(&mut out, 0, "; linker.script.v0 (textual, mínimo):");
        for script_line in freestanding_linker_script().lines() {
            line(&mut out, 0, &format!(";   {}", script_line));
        }
        line(&mut out, 0, "; kernel.stub.v0 (experimental):");
        for stub_line in freestanding_kernel_stub().lines() {
            line(&mut out, 0, &format!(";   {}", stub_line));
        }
    }
    line(&mut out, 0, ".text");

    if program.is_freestanding {
        line(
            &mut out,
            0,
            &format!(".globl {}", FREESTANDING_BOOT_ENTRY_SYMBOL),
        );
        line(&mut out, 0, &format!("{}:", FREESTANDING_BOOT_ENTRY_SYMBOL));
        line(
            &mut out,
            1,
            &format!("call {}", FREESTANDING_BOOT_ENTRY_FUNCTION),
        );
        line(&mut out, 0, ".Lpinker_hang:");
        line(&mut out, 1, "jmp .Lpinker_hang");
    }

    if !program.globals.is_empty() {
        line(&mut out, 0, ".section .rodata");
        for global in &program.globals {
            line(&mut out, 0, &format!(".globl {}", global.name));
            line(&mut out, 0, &format!("{}:", global.name));
            line(
                &mut out,
                1,
                &format!(".quad {}", render_operand(&global.value)),
            );
        }
        line(&mut out, 0, ".text");
    }

    for function in &program.functions {
        line(&mut out, 0, &format!("; abi.func {}", function.name));
        line(
            &mut out,
            0,
            &format!("; abi.params {}", render_abi_params(function)),
        );
        line(
            &mut out,
            0,
            &format!("; abi.ret {}", render_abi_return(function.ret_type)),
        );
        line(
            &mut out,
            0,
            &format!(
                "; abi.frame prologue=.L{}_prologue epilogue=.L{}_epilogue",
                function.name, function.name
            ),
        );
        line(&mut out, 0, &format!(".globl {}", function.name));
        line(&mut out, 0, &format!("{}:", function.name));
        line(&mut out, 1, &format!(".L{}_prologue:", function.name));
        line(&mut out, 2, "; abi.prologue (textual)");
        line(
            &mut out,
            1,
            &format!(
                "; slots params={} locals={}",
                join_or_empty(&function.params),
                join_or_empty(&function.locals)
            ),
        );

        for block in &function.blocks {
            line(
                &mut out,
                1,
                &format!(".L{}_{}:", function.name, block.label),
            );
            for instruction in &block.instructions {
                line(&mut out, 2, &render_instruction(instruction));
            }
            line(
                &mut out,
                2,
                &render_terminator(&block.terminator, &function.name),
            );
        }
        line(&mut out, 1, &format!(".L{}_epilogue:", function.name));
        line(&mut out, 2, "; abi.epilogue (textual)");
    }

    out
}

fn render_instruction(inst: &crate::backend_text::BackendTextInstruction) -> String {
    match inst {
        crate::backend_text::BackendTextInstruction::Mov { dest, src } => {
            format!("mov {}, {}", render_slot(dest), render_operand(src))
        }
        crate::backend_text::BackendTextInstruction::Unary { dest, op, operand } => {
            format!(
                "{} {}, {}",
                render_unary(*op),
                render_temp(*dest),
                render_operand(operand)
            )
        }
        crate::backend_text::BackendTextInstruction::Binary { dest, op, lhs, rhs } => format!(
            "{} {}, {}, {}",
            render_binop(*op),
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        crate::backend_text::BackendTextInstruction::Call {
            dest,
            callee,
            args,
            ret_type,
        } => {
            let call_site = render_call_site(callee, args);
            let abi_args = render_abi_call_args(args);

            match (dest, ret_type) {
                (Some(dest), _) => format!(
                    "{} ; abi.call {} -> {}",
                    call_site,
                    abi_args,
                    render_temp(*dest)
                ),
                (None, TypeIR::Nulo) => format!("{} ; abi.call {} -> void", call_site, abi_args),
                (None, _) => format!("; call inválida: {} {}", callee, abi_args),
            }
        }
        crate::backend_text::BackendTextInstruction::Falar { args } => format!(
            "falar {}",
            args.iter()
                .map(|arg| format!("{}:{}", render_operand(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_terminator(
    term: &crate::backend_text::BackendTextTerminator,
    function_name: &str,
) -> String {
    match term {
        crate::backend_text::BackendTextTerminator::Jump(label) => {
            format!("jmp .L{}_{}", function_name, label)
        }
        crate::backend_text::BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, .L{}_{}, .L{}_{}",
            render_operand(cond),
            function_name,
            then_label,
            function_name,
            else_label
        ),
        crate::backend_text::BackendTextTerminator::Return(Some(value)) => {
            format!("ret @ret, {}", render_operand(value))
        }
        crate::backend_text::BackendTextTerminator::Return(None) => "ret_void".to_string(),
    }
}

fn render_unary(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
        UnaryOpIR::BitNot => "bitnot",
        UnaryOpIR::Deref => "deref",
    }
}

fn render_binop(op: BinaryOpIR) -> &'static str {
    match op {
        BinaryOpIR::LogicalAnd => "and",
        BinaryOpIR::LogicalOr => "or",
        BinaryOpIR::BitAnd => "and",
        BinaryOpIR::BitOr => "or",
        BinaryOpIR::BitXor => "xor",
        BinaryOpIR::Shl => "shl",
        BinaryOpIR::Shr => "shr",
        BinaryOpIR::Add => "add",
        BinaryOpIR::Sub => "sub",
        BinaryOpIR::Mul => "mul",
        BinaryOpIR::Div => "div",
        BinaryOpIR::Mod => "mod",
        BinaryOpIR::Eq => "cmp_eq",
        BinaryOpIR::Neq => "cmp_ne",
        BinaryOpIR::Lt => "cmp_lt",
        BinaryOpIR::Lte => "cmp_le",
        BinaryOpIR::Gt => "cmp_gt",
        BinaryOpIR::Gte => "cmp_ge",
    }
}

fn render_operand(op: &crate::cfg_ir::OperandIR) -> String {
    match op {
        crate::cfg_ir::OperandIR::Local(slot) => render_slot(slot),
        crate::cfg_ir::OperandIR::GlobalConst(name) => format!("{}(%rip)", name),
        crate::cfg_ir::OperandIR::Int(v) => v.to_string(),
        crate::cfg_ir::OperandIR::Bool(v) => {
            if *v {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        crate::cfg_ir::OperandIR::Str(s) => format!("\"{}\"", s),
        crate::cfg_ir::OperandIR::Temp(temp) => render_temp(*temp),
    }
}

fn render_temp(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn render_slot(slot: &str) -> String {
    format!("${}", slot)
}

fn join_or_empty(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        values.join(", ")
    }
}

fn render_abi_params(function: &crate::backend_text::BackendTextFunction) -> String {
    if function.params.is_empty() {
        return "[]".to_string();
    }

    let rendered = function
        .params
        .iter()
        .enumerate()
        .map(|(idx, slot)| format!("@arg{}={}", idx, render_slot(slot)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", rendered)
}

fn render_abi_return(ret_type: TypeIR) -> String {
    if ret_type == TypeIR::Nulo {
        "void".to_string()
    } else {
        "@ret".to_string()
    }
}

fn render_call_site(callee: &str, args: &[crate::cfg_ir::OperandIR]) -> String {
    if args.is_empty() {
        format!("call {}", callee)
    } else {
        let args = args
            .iter()
            .map(render_operand)
            .collect::<Vec<_>>()
            .join(", ");
        format!("call {}, {}", callee, args)
    }
}

fn render_abi_call_args(args: &[crate::cfg_ir::OperandIR]) -> String {
    if args.is_empty() {
        "[]".to_string()
    } else {
        let args = args
            .iter()
            .enumerate()
            .map(|(idx, operand)| format!("@arg{}={}", idx, render_operand(operand)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{}]", args)
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

fn err(msg: &str) -> PinkerError {
    PinkerError::BackendTextValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
