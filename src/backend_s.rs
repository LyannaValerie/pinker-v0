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
use crate::native_symbol::{self, EmittedDefinitions, NativeDefinition, NativeSurface};
use crate::token::{Position, Span};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

// @pinker-nav:start backend-s.pipeline.textual-selecionado
// @pinker-nav:domain pipeline
// @pinker-nav:layer backend-s
// @pinker-nav:summary `emit_from_selected`: entrada pública do caminho `.s` textual. Recebe `&SelectedProgram`, valida o subset textual (`validate_supported_subset`), delega o lowering a `backend_text::lower_selected_program` (produz um `BackendTextProgram`) e serializa com `render_program`. Não constrói `ExternalCallConvProgram` nem emite assembly montável; a saída é a representação textual com metadados `abi.*`, distinta da representação usada pela toolchain externa.
pub fn emit_from_selected(selected: &SelectedProgram) -> Result<String, PinkerError> {
    validate_supported_subset(selected)?;
    let lowered = backend_text::lower_selected_program(selected)?;
    Ok(render_program(&lowered))
}
// @pinker-nav:end backend-s.pipeline.textual-selecionado

// @pinker-nav:start backend-s.pipeline.toolchain-externa
// @pinker-nav:domain pipeline
// @pinker-nav:layer backend-s
// @pinker-nav:summary `emit_external_toolchain_subset`: entrada pública do caminho montável hospedado. Recebe `&SelectedProgram`, constrói o `ExternalCallConvProgram` próprio via `extract_external_callconv_program` — **sem** passar por `BackendTextProgram` — e renderiza com `render_external_x86_64_linux_callconv_impl(.., false)`. Emite assembly x86-64/Linux SysV montável e ligável por toolchain externa (`cc`/`gcc`/`clang`), sem inicialização de runtime. O doc `///` do módulo enumera o subset conservador aceito.
/// Emite um `.s` mínimo montável por toolchain externa (assembler+linker do sistema).
///
/// Escopo deliberadamente mínimo para a Fase 135:
/// - target assumido: Linux x86_64 (SysV) hospedado;
/// - subset aceito: funções `-> bombom` com múltiplos blocos/labels, `jmp` incondicional, branch condicional mínimo e loop mínimo por retorno de salto entre blocos;
/// - disciplina mínima de registradores/frame: `%rax` (retorno/acumulador), `%rdi` (arg0), `%rsi` (arg1), `%rdx` (arg2), `%r10` (temporário volátil), slots em frame `%rbp`;
/// - memória mínima real garantida: load/store em slots de frame via `movq -off(%rbp), %reg` e `movq %reg, -off(%rbp)`;
/// - branch condicional mínimo via teste contra zero (`cmpq $0` + `jne`) e sem ABI completa.
/// - globais estáticas mínimas somente-leitura em `.rodata`: `eterno` de valor literal inteiro/lógico com leitura por símbolo `@nome(%rip)`.
/// - composto mínimo conservador: base homogênea `seta<bombom>` com `deref_store`/`deref_load` mínimo e abertura heterogênea em quatro camadas para `ninho`, incluindo composição mínima auditável no mesmo registro (`u32` + `u64`) via offset explícito;
/// - escalares inteiros e lógicos aceitos pela semântica em parâmetros dinâmicos, com normalização SysV explícita em registradores e pilha;
/// - `quebrar`/`continuar` (Fase 128, camada 3 conservadora) no recorte de `sempre que` já materializado em `selected`, com composição mínima auditável de três níveis de laço (`sempre que` externo/meio/interno) sem abrir subsistema geral de controle de fluxo;
/// - `virar` (Fase 134, camada 2 conservadora) no recorte mínimo explícito `u32 -> u64` e `u64 -> u32` quando a origem é slot local/parâmetro tipado;
/// - `verso` (Fase 135, camada 1 conservadora e condicional) apenas no recorte mínimo opaco: literal estático em `.rodata` + carga de endereço + tráfego por slot/parâmetro, sem operações textuais gerais.
///
/// O resultado mapeia `principal` para o símbolo `main`, para permitir linkedição
/// via driver C (`cc`/`gcc`/`clang`) sem runtime próprio.
pub fn emit_external_toolchain_subset(selected: &SelectedProgram) -> Result<String, PinkerError> {
    let program = extract_external_callconv_program(selected, false)?;
    render_external_x86_64_linux_callconv_impl(&program, false)
}
// @pinker-nav:end backend-s.pipeline.toolchain-externa

// @pinker-nav:start backend-s.pipeline.nativo-runtime
// @pinker-nav:domain pipeline
// @pinker-nav:layer backend-s
// @pinker-nav:summary `emit_external_toolchain_subset_nativo`: entrada pública do caminho de build nativo. Usa a **mesma** representação externa (`extract_external_callconv_program`) do caminho hospedado, mas renderiza com `render_external_x86_64_linux_callconv_impl(.., true)`, habilitando a chamada a `pinker_rt_iniciar` no prólogo de `main`. Emite referências a símbolos resolvidos por `libpinker_rt.a`; o runtime não é implementado neste arquivo.
/// Variante nativa do subset externo (Eixo B do Bloco 20, fase B1): o `main`
/// gerado chama `pinker_rt_iniciar(argc, argv)` no prólogo, exigindo link com
/// a staticlib `libpinker_rt.a` do workspace. É o caminho usado por
/// `pink build --nativo`.
pub fn emit_external_toolchain_subset_nativo(
    selected: &SelectedProgram,
) -> Result<String, PinkerError> {
    let program = extract_external_callconv_program(selected, true)?;
    render_external_x86_64_linux_callconv_impl(&program, true)
}
// @pinker-nav:end backend-s.pipeline.nativo-runtime

// @pinker-nav:start backend-s.validacao.subset-textual
// @pinker-nav:domain validacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `validate_supported_subset`: validação do subset aceito **apenas** pelo caminho `.s` textual (`emit_from_selected`). Percorre funções recusando retorno, tipo de slot e retorno de `call` fora de `is_supported_type` (`bombom`, inteiros `u8..i64`, `logica`, `nulo`). É independente das validações incorporadas em `extract_external_callconv_program` (caminho montável), que aceitam um conjunto distinto de tipos (`verso`, listas, mapas, `seta<T>`, `ninho`).
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
// @pinker-nav:end backend-s.validacao.subset-textual

// @pinker-nav:start backend-s.modelo.callconv-externa
// @pinker-nav:domain modelo
// @pinker-nav:layer backend-s
// @pinker-nav:summary Modelo intermediário do caminho montável: `ExternalCallConvProgram` (globais de `.rodata`, strings de `.rodata`, funções) e seus componentes — `ExternalCallConvGlobal` (nome + valor `u64`), `ExternalCallConvString` (label + valor), `ExternalCallConvFunction` (nome, `stack_size`, `slot_offsets`, blocos, parâmetros) e `ExternalCallConvBlock` (label, `body: Vec<String>`, terminador) com o enum `ExternalCallConvTerminator` (`Jmp`/`Br`/`Ret`). **Não** é `BackendTextProgram`: os corpos dos blocos já são linhas de assembly textualizadas (`Vec<String>`), perdendo a estrutura semântica original (tipos, spans, temporários estruturados). Não há alocador geral de registradores; os papéis de registrador são fixos.
struct ExternalCallConvProgram {
    rodata_globals: Vec<ExternalCallConvGlobal>,
    rodata_strings: Vec<ExternalCallConvString>,
    // Fase 242: nomes de função referenciados como valor callable em algum
    // ponto do programa; cada um recebe um descritor estático
    // {code_ptr, env_ptr} em `.rodata` (env_ptr nulo — não capturante).
    rodata_function_refs: Vec<String>,
    trait_vtables: Vec<ExternalTraitVtable>,
    trait_adapters: Vec<ExternalTraitAdapter>,
    functions: Vec<ExternalCallConvFunction>,
}

struct ExternalCallConvGlobal {
    name: String,
    value: u64,
}

struct ExternalCallConvString {
    label: String,
    value: String,
}

#[derive(Clone, PartialEq, Eq)]
struct ExternalTraitVtable {
    symbol: String,
    entries: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct ExternalTraitAdapter {
    symbol: String,
    target: String,
    concrete_type: TypeIR,
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
    RetVoid,
}
// @pinker-nav:end backend-s.modelo.callconv-externa

// @pinker-nav:start backend-s.abi.registradores-argumentos
// @pinker-nav:domain abi
// @pinker-nav:layer backend-s
// @pinker-nav:summary Papéis fixos de registrador na ABI SysV x86-64 do caminho montável: `REG_RET` (`%rax`, retorno/acumulador), `ARG_REGS` (os 6 registradores de argumento `%rdi`/`%rsi`/`%rdx`/`%rcx`/`%r8`/`%r9`) e `REG_TMP` (`%r10`, temporário). Argumentos a partir do 7º viajam pela pilha, com padding para o alinhamento de 16 bytes no `call`. Não há alocação dinâmica de registradores; os papéis são codificados diretamente no arquivo.
const REG_RET: &str = "%rax";
// ABI SysV x86-64 completa (Fase 213/B2): 6 registradores de argumento;
// argumentos adicionais viajam pela pilha (7º em diante), com padding para
// manter o alinhamento de 16 bytes exigido no `call`.
const ARG_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
const REG_TMP: &str = "%r10";
// @pinker-nav:end backend-s.abi.registradores-argumentos

// @pinker-nav:start backend-s.lowering.globais-rodata
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-s
// @pinker-nav:summary `extract_external_callconv_program` (início): deduplicação de símbolos globais (recusa duplicados), aceitação apenas de globais estáticas `bombom`/`logica` com inicializador literal inteiro/lógico (`OperandIR::Int`/`Bool`), montagem de `rodata_globals`, e a exigência de função `principal`. Primeira responsabilidade contígua da extração para `ExternalCallConvProgram`.
fn extract_external_callconv_program(
    selected: &SelectedProgram,
    native_runtime: bool,
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

    let has_main = selected
        .functions
        .iter()
        .any(|f| native_symbol::is_entrypoint(&f.name));
    if !has_main {
        return Err(err(
            "subset externo montável (Fase 84) exige função `principal`",
        ));
    }
    // @pinker-nav:end backend-s.lowering.globais-rodata

    // @pinker-nav:start backend-s.lowering.funcoes-frames
    // @pinker-nav:domain lowering
    // @pinker-nav:layer backend-s
    // @pinker-nav:summary Validação e enquadramento por função no caminho montável: recusa de retorno fora de `is_external_ret_type`, `principal` sem parâmetros, tipos de parâmetro/local fora de `is_external_param_type`/`is_external_local_type`, exigência de ao menos um bloco e `validate_external_block_labels`. Em seguida constrói `slot_offsets` alocando 8 bytes por slot na ordem parâmetros → locais → temporários (`collect_temp_ids`), calcula `raw_stack` e arredonda `stack_size` para múltiplo de 16 (0 quando não há slots). Tipos menores ainda ocupam slot de 8 bytes.
    let mut functions = Vec::new();
    let mut rodata_string_labels = HashMap::new();
    let mut rodata_strings = Vec::new();
    let mut trait_vtables = BTreeMap::<String, ExternalTraitVtable>::new();
    let mut trait_adapters = BTreeMap::<String, ExternalTraitAdapter>::new();
    for function in &selected.functions {
        if !is_external_ret_type(&function.ret_type) {
            return Err(err(
                "subset externo montável (Fase 215) aceita retorno `bombom`, `verso` ou `logica` em funções",
            ));
        }
        if native_symbol::is_entrypoint(&function.name) && !function.params.is_empty() {
            return Err(err(
                "subset externo montável (Fase 84) exige `principal()` sem parâmetros",
            ));
        }
        for (param_index, param) in function.params.iter().enumerate() {
            let Some(ty) = function.slot_types.get(param) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou parâmetro sem tipo",
                ));
            };
            let is_trait_receiver = param_index == 0
                && function.name.starts_with("__impl_")
                && is_external_trait_receiver_type(ty);
            let is_trait_method_word =
                function.name.starts_with("__impl_") && ty.is_native_abi_word();
            let supported_param = if native_runtime {
                is_external_param_type(ty) || is_external_scalar_param_type(ty)
            } else {
                is_external_param_type(ty)
            };
            if !supported_param && !is_trait_receiver && !is_trait_method_word {
                return Err(err(
                    "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>` no recorte conservador",
                ));
            }
        }
        for local in &function.locals {
            let Some(ty) = function.slot_types.get(local) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou local sem tipo",
                ));
            };
            let is_trait_snapshot_source = function.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        SelectedInstr::MakeTraitObject {
                            value: OperandIR::Local(slot),
                            concrete_type,
                            ..
                        } if slot == local && concrete_type == ty
                    )
                })
            });
            let is_trait_method_word =
                function.name.starts_with("__impl_") && ty.is_native_abi_word();
            if !(is_external_local_type(ty)
                || is_trait_method_word
                || is_trait_snapshot_source && is_external_trait_receiver_type(ty))
            {
                return Err(err(&format!(
                    "subset externo montável só aceita local `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`; '{}' é '{}'",
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
        // HR3: as operações de união deixam de presumir que todo storage ocupa
        // oito bytes. Cada injeção recebe um scratch do tamanho real do payload
        // e cada extração recebe storage próprio para o binding, ambos
        // alinhados. Os offsets são múltiplos de 16, o maior alinhamento
        // suportado por `MAX_UNION_PAYLOAD_ALIGN`, e crescem com aritmética
        // checada.
        let mut union_storage_offsets: HashMap<UnionStorageKey, u32> = HashMap::new();
        let mut frame_top = (slot_index.saturating_sub(1)) * 8;
        frame_top = frame_top.div_ceil(16) * 16;
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instr_index, inst) in block.instructions.iter().enumerate() {
                let layout = match inst {
                    SelectedInstr::UnionInject { payload_layout, .. }
                    | SelectedInstr::UnionExtract { payload_layout, .. } => *payload_layout,
                    _ => continue,
                };
                if !layout.is_well_formed() {
                    return Err(err(
                        "subset externo montável recusa layout de payload de união mal formado",
                    ));
                }
                let bytes = u32::try_from(layout.size).map_err(|_| {
                    err("subset externo montável recusa payload de união acima da plataforma")
                })?;
                let reserved = bytes.div_ceil(16).saturating_mul(16);
                frame_top = frame_top.checked_add(reserved).ok_or_else(|| {
                    err("overflow no frame do subset externo montável ao reservar storage de união")
                })?;
                union_storage_offsets.insert(
                    UnionStorageKey {
                        block: block_index,
                        instr: instr_index,
                    },
                    frame_top,
                );
            }
        }
        let raw_stack = frame_top;
        let stack_size = if raw_stack == 0 {
            0
        } else {
            raw_stack.div_ceil(16) * 16
        };
        // @pinker-nav:end backend-s.lowering.funcoes-frames

        // @pinker-nav:start backend-s.lowering.blocos-terminadores
        // @pinker-nav:domain lowering
        // @pinker-nav:layer backend-s
        // @pinker-nav:summary Abertura do laço de blocos e seleção do terminador de cada bloco: `SelectedTerminator::Jmp` → `ExternalCallConvTerminator::Jmp`; `Ret(Some(value))` materializa literais `verso` de retorno em `.rodata` (`register_rodata_strings_for_operand`) e vira `Ret`; `Ret(None)` vira `RetVoid` para funções/métodos `nulo`; `Br` copia condição e rótulos. Constrói o `terminator` antes do corpo do bloco.
        let mut blocks = Vec::new();
        // Identificador determinístico de envelope de `sussurro` dentro da função.
        let mut inline_asm_envelopes = 0_u32;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let terminator = match &block.terminator {
                SelectedTerminator::Jmp(target) => ExternalCallConvTerminator::Jmp(target.clone()),
                SelectedTerminator::Ret(Some(value)) => {
                    // Literais `verso` também podem aparecer direto no retorno
                    // (`mimo "texto";`); materializa o rodata aqui (Fase 215/B4).
                    register_rodata_strings_for_operand(
                        value,
                        &mut rodata_string_labels,
                        &mut rodata_strings,
                    );
                    ExternalCallConvTerminator::Ret(value.clone())
                }
                SelectedTerminator::Ret(None) => ExternalCallConvTerminator::RetVoid,
                SelectedTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => ExternalCallConvTerminator::Br {
                    cond: cond.clone(),
                    then_label: then_label.clone(),
                    else_label: else_label.clone(),
                },
            };
            // @pinker-nav:end backend-s.lowering.blocos-terminadores

            // @pinker-nav:start backend-s.lowering.operacoes-memoria
            // @pinker-nav:domain lowering
            // @pinker-nav:layer backend-s
            // @pinker-nav:summary Lowering externo de dados/memória: `Mov`; aritmética `Add`/`Sub`/`Mul`, com validação nativa de derivação quando o resultado preserva tipo ponteiro; comparações, incluindo condições assinadas inferidas dos produtores; `DerefLoad`/`DerefStore` por largura e sinal para todos os escalares públicos, precedidos por validação de região; e casts de uma palavra. O caminho hospedado legado mantém seu subconjunto conservador.
            let mut body = Vec::new();
            for (instr_index, inst) in block.instructions.iter().enumerate() {
                match inst {
                    SelectedInstr::Mov { dest, src } => {
                        ensure_dest_is_local_or_param(dest, function)?;
                        register_rodata_strings_for_operand(
                            src,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, src, &slot_offsets, &rodata_strings)?);
                        body.push(format!("movq {}, -{}(%rbp)", REG_RET, slot_offsets[dest]));
                    }
                    SelectedInstr::Neg { dest, operand, ty } => {
                        register_rodata_strings_for_operand(
                            operand,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(
                            REG_RET,
                            operand,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("negq {}", REG_RET));
                        body.extend(normalize_rax(*ty));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::PointerOffset {
                        dest,
                        pointer,
                        offset,
                        element_size,
                        element_align,
                        ..
                    } => {
                        body.extend(lower_typed_pointer_offset(
                            function,
                            *dest,
                            pointer,
                            offset,
                            (*element_size, *element_align),
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Add { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "addq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                        body.extend(lower_public_pointer_derivation(
                            function,
                            *dest,
                            lhs,
                            rhs,
                            false,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Sub { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "subq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                        body.extend(lower_public_pointer_derivation(
                            function,
                            *dest,
                            lhs,
                            rhs,
                            true,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Mul { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "imulq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitAnd { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "andq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitOr { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "orq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitXor { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "xorq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::Shl { dest, lhs, rhs, ty }
                    | SelectedInstr::Shr { dest, lhs, rhs, ty } => {
                        body.extend(lower_shift(
                            matches!(inst, SelectedInstr::Shr { .. }),
                            *dest,
                            lhs,
                            rhs,
                            *ty,
                            &function.name,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::Div { dest, lhs, rhs, ty }
                    | SelectedInstr::Mod { dest, lhs, rhs, ty } => {
                        body.extend(lower_div_mod(
                            matches!(inst, SelectedInstr::Mod { .. }),
                            *dest,
                            lhs,
                            rhs,
                            *ty,
                            &function.name,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpEq { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_eq(
                            *dest,
                            lhs,
                            rhs,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpNe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_ne(
                            *dest,
                            lhs,
                            rhs,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpLt { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_lt(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpGt { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_gt(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpLe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_le(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpGe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_ge(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::DerefLoad {
                        dest,
                        ptr,
                        ty,
                        is_volatile,
                    } => {
                        // HR3: um agregado é representado **pelo endereço** da
                        // sua representação completa. Abrir `*ptr` de um array
                        // fixo ou de um `ninho` não lê memória: produz o mesmo
                        // endereço, que é o que a injeção de união entrega ao
                        // runtime para a cópia integral.
                        if matches!(ty, TypeIR::FixedArray { .. } | TypeIR::Struct) {
                            if *is_volatile {
                                return Err(err(
                                    "subset externo montável não suporta caminho `fragil` em agregado",
                                ));
                            }
                            register_rodata_strings_for_operand(
                                ptr,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                REG_RET,
                                ptr,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        if !(if native_runtime {
                            is_external_deref_load_type(ty)
                        } else {
                            is_external_legacy_deref_load_type(ty)
                        }) {
                            return Err(err(
                                "subset externo montável (Fase 134) aceita `deref_load` apenas no recorte mínimo `bombom`/`u32`/`u64` (camada 4 conservadora de `ninho` heterogêneo + legado homogêneo)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 134) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            ptr,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        let mut visiting_temps = HashSet::new();
                        let mut visiting_slots = HashSet::new();
                        if selected_operand_is_public_pointer(
                            function,
                            ptr,
                            &mut visiting_temps,
                            &mut visiting_slots,
                        ) {
                            body.push(format!("movq {}, %rdi", REG_RET));
                            body.push(format!("movq ${}, %rsi", external_memory_width(*ty)));
                            body.push(format!("movq ${}, %rdx", external_memory_alignment(*ty)));
                            body.push("call pinker_publico_validar_acesso".to_string());
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        body.push(if native_runtime {
                            match ty {
                                TypeIR::U8 | TypeIR::Logica => {
                                    format!("movzbq ({}), {}", REG_RET, REG_RET)
                                }
                                TypeIR::I8 => format!("movsbq ({}), {}", REG_RET, REG_RET),
                                TypeIR::U16 => format!("movzwq ({}), {}", REG_RET, REG_RET),
                                TypeIR::I16 => format!("movswq ({}), {}", REG_RET, REG_RET),
                                TypeIR::U32 => format!("movl ({}), %eax", REG_RET),
                                TypeIR::I32 => format!("movslq ({}), {}", REG_RET, REG_RET),
                                _ => format!("movq ({}), {}", REG_RET, REG_RET),
                            }
                        } else {
                            format!("movq ({}), {}", REG_RET, REG_RET)
                        });
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
                        if !(if native_runtime {
                            is_external_deref_store_type(ty)
                        } else {
                            is_external_legacy_deref_store_type(ty)
                        }) {
                            return Err(err(
                                "subset externo montável (Fase 134) aceita `deref_store` apenas no recorte mínimo `bombom`/`u32`/`u64` (camada 4 conservadora de `ninho` heterogêneo + legado homogêneo)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 134) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            ptr,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        let mut visiting_temps = HashSet::new();
                        let mut visiting_slots = HashSet::new();
                        if selected_operand_is_public_pointer(
                            function,
                            ptr,
                            &mut visiting_temps,
                            &mut visiting_slots,
                        ) {
                            body.push(format!("movq {}, %rdi", REG_RET));
                            body.push(format!("movq ${}, %rsi", external_memory_width(*ty)));
                            body.push(format!("movq ${}, %rdx", external_memory_alignment(*ty)));
                            body.push("call pinker_publico_validar_acesso".to_string());
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        body.extend(load_operand(
                            REG_TMP,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(if native_runtime {
                            match ty {
                                TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => {
                                    format!("movb %r10b, ({})", REG_RET)
                                }
                                TypeIR::U16 | TypeIR::I16 => {
                                    format!("movw %r10w, ({})", REG_RET)
                                }
                                TypeIR::U32 | TypeIR::I32 => {
                                    format!("movl %r10d, ({})", REG_RET)
                                }
                                _ => format!("movq {}, ({})", REG_TMP, REG_RET),
                            }
                        } else {
                            format!("movq {}, ({})", REG_TMP, REG_RET)
                        });
                    }
                    SelectedInstr::Cast {
                        dest,
                        value,
                        target_type,
                    } => {
                        if matches!(target_type, TypeIR::Pointer { .. }) {
                            register_rodata_strings_for_operand(
                                value,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                REG_RET,
                                value,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        if !target_type.is_integer() {
                            return Err(err(
                                "backend nativo aceita `virar` escalar apenas para inteiro ou ponteiro",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(
                            REG_RET,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.extend(normalize_rax(*target_type));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // @pinker-nav:end backend-s.lowering.operacoes-memoria

                    // @pinker-nav:start backend-s.lowering.chamadas-sysv
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Lowering de chamadas no corpo do bloco (ABI SysV): `Call` com destino trata `__ternario` puro por `cmoveq`; `formatar_verso` materializa um pack contíguo de handles `verso` na pilha e chama a autoridade única `pinker_formatar_verso_pack(modelo,count,entries)`; passagem dos 6 primeiros argumentos em `ARG_REGS`, empilhamento do 7º+ do último ao primeiro com padding de alinhamento e cleanup após o `call`. `Call` e `CallVoid` delegam a escolha do destino a `resolver_rota_de_chamada`, a autoridade única de seleção, e diferem apenas em `CallVoid` não guardar `%rax`; aridade fora do recorte e callee desconhecido continuam recusados por esta camada.
                    SelectedInstr::Call {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        if !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "subset externo montável (Fase 216) só aceita call com retorno `bombom`, `verso`, `logica`, lista ou `nulo`",
                            ));
                        }
                        // A CFG conserva a pseudo-chamada somente quando os
                        // dois braços são valores trivialmente puros. Braços
                        // com chamadas, alocações ou outros efeitos já foram
                        // separados em blocos lazy antes da seleção.
                        if callee == "__ternario" {
                            if args.len() != 3 {
                                return Err(err(
                                    "subset externo montável (Fase 214) exige `__ternario` com 3 argumentos",
                                ));
                            }
                            for arg in args {
                                register_rodata_strings_for_operand(
                                    arg,
                                    &mut rodata_string_labels,
                                    &mut rodata_strings,
                                );
                            }
                            body.extend(load_operand(
                                REG_RET,
                                &args[1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(load_operand(
                                "%r10",
                                &args[2],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(load_operand(
                                "%r11",
                                &args[0],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push("cmpq $0, %r11".to_string());
                            body.push(format!("cmoveq %r10, {}", REG_RET));
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        // D7: a IR já converte todos os argumentos de
                        // substituição para handles `verso`. Materializamos
                        // um pack contíguo e passamos modelo/count/entries à
                        // autoridade única do runtime, independentemente da
                        // quantidade de argumentos.
                        if callee == "formatar_verso" {
                            if args.len() < 2 {
                                return Err(err(
                                    "subset externo montável exige ao menos uma substituição em formatar_verso",
                                ));
                            }
                            for arg in args {
                                register_rodata_strings_for_operand(
                                    arg,
                                    &mut rodata_string_labels,
                                    &mut rodata_strings,
                                );
                            }
                            let substitutions = args.len() - 1;
                            let pack_bytes = substitutions.checked_mul(8).ok_or_else(|| {
                                err("pack de formatar_verso excede a representação da plataforma")
                            })?;
                            if pack_bytes > isize::MAX as usize {
                                return Err(err(
                                    "pack de formatar_verso excede a representação da plataforma",
                                ));
                            }
                            let pad_words = substitutions % 2;
                            if pad_words == 1 {
                                body.push("subq $8, %rsp".to_string());
                            }
                            for arg in args.iter().skip(1).rev() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    arg,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("pushq {}", REG_TMP));
                            }
                            body.extend(load_operand(
                                ARG_REGS[0],
                                &args[0],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("movq ${}, {}", substitutions, ARG_REGS[1]));
                            body.push(format!("movq %rsp, {}", ARG_REGS[2]));
                            body.push("call pinker_formatar_verso_pack".to_string());
                            let cleanup_bytes = pack_bytes
                                .checked_add(pad_words * 8)
                                .filter(|bytes| *bytes <= isize::MAX as usize)
                                .ok_or_else(|| {
                                    err("pack de formatar_verso excede a representação da plataforma")
                                })?;
                            if cleanup_bytes > 0 {
                                body.push(format!("addq ${}, %rsp", cleanup_bytes));
                            }
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        // Intrínsecas de aridade variável usam wrappers por
                        // aridade no runtime (Fases 219/B8 e 221/B10).
                        let call_target = match resolver_rota_de_chamada(callee, args.len(), || {
                            selected.functions.iter().any(|f| &f.name == callee)
                        }) {
                            RotaDeChamada::Runtime(simbolo) => simbolo,
                            RotaDeChamada::FuncaoPinker(simbolo) => simbolo,
                            RotaDeChamada::AridadeForaDoRecorte => {
                                return Err(err(
                                    "subset externo montável (Fase 221) recusa aridade fora do recorte da intrínseca de runtime",
                                ));
                            }
                            RotaDeChamada::CalleeDesconhecido => {
                                return Err(err(
                                    "subset externo montável (Fase 84) encontrou call para função inexistente",
                                ));
                            }
                        };
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        // ABI SysV completa (Fase 213/B2): 7º argumento em
                        // diante viaja pela pilha, empilhado do último para o
                        // primeiro; padding mantém o alinhamento de 16 no call.
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand("%r10", arg, &slot_offsets, &rodata_strings)?);
                            body.push("pushq %r10".to_string());
                        }
                        for (idx, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[idx],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.push(format!("call {}", call_target));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // Fase 242: chamada indireta real. `callee` é um operando
                    // (handle callable: endereço do descritor estático ou
                    // heap {code_ptr, env_ptr}), não um símbolo. Mesma ABI de
                    // argumentos do usuário do `call` direto; o código é lido
                    // do descritor em tempo de execução e chamado via
                    // registrador (`call *reg`). `env_ptr` (offset 8) é
                    // reservado/ignorado nesta fase (sem captura).
                    SelectedInstr::CallIndirect {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        if !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "subset externo montável (Fase 242) só aceita call_indirect com retorno `bombom`, `verso`, `logica`, lista ou carinho",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            callee,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        // Fase 243: `__env` é sempre o argumento real final
                        // (trailing) — uniforme para toda função
                        // indiretamente chamável, capturante ou não (ver
                        // `ir.rs::lower_closure_function`/`ensure_fnref_wrapper`).
                        // O índice virtual `args.len()` é sempre `__env`,
                        // extraído do descritor (offset 8) em vez de vir de
                        // um operando real.
                        let total_args = args.len() + 1;
                        let stack_args = total_args.saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for index in (ARG_REGS.len()..total_args).rev() {
                            if index == args.len() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    callee,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("movq 8({0}), {0}", REG_TMP));
                                body.push(format!("pushq {}", REG_TMP));
                            } else {
                                body.extend(load_operand(
                                    "%r11",
                                    &args[index],
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push("pushq %r11".to_string());
                            }
                        }
                        for index in 0..total_args.min(ARG_REGS.len()) {
                            if index == args.len() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    callee,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("movq 8({}), {}", REG_TMP, ARG_REGS[index]));
                            } else {
                                body.extend(load_operand(
                                    ARG_REGS[index],
                                    &args[index],
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                            }
                        }
                        // code_ptr por último: recarrega o handle (slot
                        // estável, seguro reler) — não conflita com o uso
                        // anterior de REG_TMP para extrair env_ptr, já
                        // consumido acima nos ramos de pilha/registrador.
                        body.extend(load_operand(
                            REG_TMP,
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("movq ({}), {}", REG_TMP, REG_TMP));
                        body.push(format!("call *{}", REG_TMP));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // Fase 245: endereço cru de código em uma palavra. A ABI
                    // contém apenas os argumentos declarados, sem descritor e
                    // sem o argumento implícito `__env`.
                    SelectedInstr::CallRaw {
                        dest,
                        callee,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        if args.len() != param_types.len()
                            || !param_types.iter().all(is_external_raw_call_type)
                            || !is_external_raw_call_ret_type(ret_type)
                        {
                            return Err(err(
                                "subset externo montável encontrou assinatura ABI inválida em call_raw",
                            ));
                        }
                        for arg in args {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        body.extend(load_operand(
                            "%rdi",
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push("call pinker_publico_validar_ponteiro_funcao".to_string());
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand(
                                REG_TMP,
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("pushq {}", REG_TMP));
                        }
                        for (index, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[index],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.extend(load_operand(
                            REG_TMP,
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("call *{}", REG_TMP));
                        if stack_args > 0 || pad > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        match (dest, ret_type) {
                            (Some(_), TypeIR::Nulo) => {
                                return Err(err("call_raw nulo não pode ter destino"));
                            }
                            (Some(dest), _) => body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            )),
                            (None, TypeIR::Nulo) => {}
                            (None, _) => {
                                return Err(err("call_raw com retorno exige destino"));
                            }
                        }
                    }
                    // Fase 243/D3: materializa uma closure em uma única
                    // alocação possuída pelo descritor dinâmico
                    // {code_ptr, env_ptr}. O ambiente, quando existe, ocupa o
                    // storage trailing (uma palavra por captura), eliminando
                    // a falha parcial entre ambiente e descritor. Continua
                    // distinto do descritor ESTÁTICO em `.rodata` de
                    // `FunctionRef`.
                    SelectedInstr::MakeClosure {
                        dest,
                        function_name,
                        captures,
                    } => {
                        if !selected.functions.iter().any(|f| &f.name == function_name) {
                            return Err(err(
                                "subset externo montável (Fase 243) encontrou make_closure para função inexistente",
                            ));
                        }
                        for capture in captures.iter() {
                            register_rodata_strings_for_operand(
                                capture,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        let dest_offset = slot_offsets[&temp_key(*dest)];
                        body.push(format!("movabsq ${}, %rdi", captures.len()));
                        body.push("call pinker_callable_alocar".to_string());
                        // O slot já recebe a identidade final do descritor;
                        // `8(descriptor)` contém o ambiente possuído.
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                        for (index, capture) in captures.iter().enumerate() {
                            body.push(format!("movq -{}(%rbp), %r10", dest_offset));
                            body.push("movq 8(%r10), %r10".to_string());
                            body.extend(load_operand(
                                "%r11",
                                capture,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("movq %r11, {}(%r10)", index * 8));
                        }
                        body.push(format!("movq -{}(%rbp), %r11", dest_offset));
                        body.push(format!("leaq {}(%rip), %r10", function_name));
                        body.push("movq %r10, (%r11)".to_string());
                    }
                    // Call sem destino (intrínsecas de efeito, Fase 216/B5):
                    // mesma ABI do call comum, sem o movq de retorno.
                    SelectedInstr::CallVoid { callee, args } => {
                        let call_target = match resolver_rota_de_chamada(callee, args.len(), || {
                            selected.functions.iter().any(|f| &f.name == callee)
                        }) {
                            RotaDeChamada::Runtime(simbolo) => simbolo,
                            RotaDeChamada::FuncaoPinker(simbolo) => simbolo,
                            RotaDeChamada::AridadeForaDoRecorte => {
                                return Err(err(
                                    "subset externo montável (Fase 221) recusa aridade fora do recorte da intrínseca de runtime",
                                ));
                            }
                            RotaDeChamada::CalleeDesconhecido => {
                                return Err(err(
                                    "subset externo montável (Fase 84) encontrou call para função inexistente",
                                ));
                            }
                        };
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand("%r10", arg, &slot_offsets, &rodata_strings)?);
                            body.push("pushq %r10".to_string());
                        }
                        for (idx, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[idx],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.push(format!("call {}", call_target));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                    }
                    // @pinker-nav:end backend-s.lowering.chamadas-sysv
                    // @pinker-nav:start backend-s.lowering.objetos-trato-nativos
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Materialização nativa de `trato<T>` e despacho por vtable: `MakeTraitObject` avalia o operando uma vez, aloca/copia o snapshot pelo tamanho concreto exato, aloca o descritor `{data_ptr,vtable_ptr}` de 16 bytes e guarda seu endereço no destino. `TraitCall` carrega descritor, snapshot, vtable e slot, posiciona receiver + argumentos pela ABI SysV (incluindo spill/padding) e executa `call *%r11`, sem `__env`; retorno `nulo` não grava destino.
                    SelectedInstr::MakeTraitObject {
                        dest,
                        value,
                        trait_name,
                        concrete_type,
                        concrete_type_name,
                        concrete_size,
                        vtable_methods,
                    } => {
                        let vtable_symbol = register_trait_vtable(
                            selected,
                            trait_name,
                            concrete_type_name,
                            *concrete_type,
                            vtable_methods,
                            &mut trait_vtables,
                            &mut trait_adapters,
                        )?;
                        body.extend(load_operand(
                            REG_RET,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);

                        // Preserva o operando através da primeira alocação sem
                        // depender de registrador caller-saved e mantém %rsp
                        // alinhado a 16 bytes antes do call.
                        body.push("pushq %rax".to_string());
                        body.push("subq $8, %rsp".to_string());
                        body.push(format!("movabsq ${}, %rdi", concrete_size));
                        body.push("call pinker_alocar".to_string());
                        body.push("addq $8, %rsp".to_string());
                        body.push("popq %r10".to_string());
                        body.extend(lower_trait_snapshot_copy(*concrete_type, *concrete_size)?);

                        let dest_offset = slot_offsets[&temp_key(*dest)];
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                        body.push("movabsq $16, %rdi".to_string());
                        body.push("call pinker_alocar".to_string());
                        body.push(format!("movq -{}(%rbp), %r10", dest_offset));
                        body.push("movq %r10, 0(%rax)".to_string());
                        body.push(format!("leaq {}(%rip), %r10", vtable_symbol));
                        body.push("movq %r10, 8(%rax)".to_string());
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                    }
                    SelectedInstr::TraitCall {
                        dest,
                        object,
                        trait_name: _,
                        method_name: _,
                        method_slot,
                        method_count,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        if *method_count == 0 || *method_slot >= *method_count {
                            return Err(err(
                                "backend nativo encontrou slot de chamada dinâmica fora da vtable",
                            ));
                        }
                        if param_types.len() != args.len()
                            || param_types.iter().any(|ty| !ty.is_native_abi_word())
                        {
                            return Err(err(
                                "backend nativo encontrou assinatura de chamada dinâmica fora do subset SysV",
                            ));
                        }
                        if *ret_type != TypeIR::Nulo && !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "backend nativo encontrou retorno de chamada dinâmica fora do subset SysV",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            object,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        for arg in args {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }

                        // O handle é carregado exatamente uma vez. Método e
                        // data_ptr ficam em dois spills internos, abaixo dos
                        // argumentos SysV escritos pelo usuário.
                        body.extend(load_operand(
                            "%r10",
                            object,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push("movq 0(%r10), %r11".to_string());
                        body.push("movq 8(%r10), %r10".to_string());
                        body.push(format!("movq {}(%r10), %r10", method_slot * 8));
                        body.push("pushq %r10".to_string());
                        body.push("pushq %r11".to_string());

                        let total_args = args.len() + 1;
                        let (stack_args, pad) = sysv_stack_layout(total_args);
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for virtual_index in (ARG_REGS.len()..total_args).rev() {
                            let param_type = param_types[virtual_index - 1];
                            body.extend(load_operand(
                                "%r11",
                                &args[virtual_index - 1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(normalize_sysv_scalar_argument("%r11", param_type)?);
                            body.push("pushq %r11".to_string());
                        }

                        let data_offset = 8 * (stack_args + pad);
                        body.push(format!("movq {}(%rsp), %rdi", data_offset));
                        for virtual_index in 1..total_args.min(ARG_REGS.len()) {
                            let param_type = param_types[virtual_index - 1];
                            body.extend(load_operand(
                                ARG_REGS[virtual_index],
                                &args[virtual_index - 1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(normalize_sysv_scalar_argument(
                                ARG_REGS[virtual_index],
                                param_type,
                            )?);
                        }
                        body.push(format!("movq {}(%rsp), %r11", data_offset + 8));
                        body.push("call *%r11".to_string());
                        body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad + 2)));
                        if let Some(dest) = dest {
                            body.push(format!(
                                "movq %rax, -{}(%rbp)",
                                slot_offsets[&temp_key(*dest)]
                            ));
                        }
                    }
                    // @pinker-nav:end backend-s.lowering.objetos-trato-nativos

                    // @pinker-nav:start backend-s.lowering.falar-runtime
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Lowering de `falar` no corpo do bloco: cada pedaço vira uma chamada ao runtime conforme o tipo (`pinker_falar_pedaco_verso`/`_logica`/`_bombom`), com `pinker_falar_espaco` como separador entre pedaços e `pinker_falar_fim` ao final — espelhando `PrintInt`/`PrintBool`/`PrintStr` do interpretador. Inclui o braço catch-all do `match` que recusa instruções fora do subset montável. `falar` continua instrução própria (não intrínseca); mesmo o caminho hospedado (não nativo) emite referências a esses símbolos de `pinker_rt` quando o programa usa `falar` ou intrínsecas.
                    // `falar` nativo (Fase 215/B4): cada pedaço vira uma
                    // chamada ao runtime conforme o tipo, com separador entre
                    // pedaços e quebra de linha ao final — espelhando as
                    // instruções PrintInt/PrintBool/PrintStr do interpretador.
                    SelectedInstr::Falar { args } => {
                        for (idx, arg) in args.iter().enumerate() {
                            if idx > 0 {
                                body.push("call pinker_falar_espaco".to_string());
                            }
                            register_rodata_strings_for_operand(
                                &arg.value,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                ARG_REGS[0],
                                &arg.value,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            let pedaco = match arg.ty {
                                TypeIR::Verso => "pinker_falar_pedaco_verso",
                                TypeIR::Logica => "pinker_falar_pedaco_logica",
                                TypeIR::I8 | TypeIR::I16 | TypeIR::I32 | TypeIR::I64 => {
                                    "pinker_falar_pedaco_inteiro"
                                }
                                _ => "pinker_falar_pedaco_bombom",
                            };
                            body.push(format!("call {}", pedaco));
                        }
                        body.push("call pinker_falar_fim".to_string());
                    }
                    SelectedInstr::InlineAsm {
                        chunks,
                        operands,
                        clobbers,
                        ..
                    } => {
                        let constraint_specs = operands
                            .iter()
                            .map(|operand| match operand {
                                crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                                    name,
                                    constraint,
                                    ..
                                }
                                | crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                                    name,
                                    constraint,
                                    ..
                                } => (name.clone(), *constraint),
                            })
                            .collect::<Vec<_>>();
                        let bindings =
                            crate::inline_asm::allocate_registers(&constraint_specs, clobbers)
                                .map_err(|error| err(&error.to_string()))?;
                        for operand in operands {
                            if let crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                                name,
                                value,
                                ty,
                                ..
                            } = operand
                            {
                                let register = bindings[name].att();
                                body.push(format!(
                                    "# pinker:sussurro input {name} -> {}",
                                    bindings[name].intel()
                                ));
                                body.extend(load_operand(
                                    register,
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.extend(normalize_sysv_scalar_argument(register, *ty)?);
                            }
                        }
                        // As sentinelas do envelope são geradas pelo compilador;
                        // nenhum texto delas vem da fonte. O identificador é
                        // determinístico por função e ordem de bloco.
                        inline_asm_envelopes += 1;
                        let envelope_id = format!("{}#{}", function.name, inline_asm_envelopes);
                        body.push(format!(
                            "{}{envelope_id}",
                            crate::inline_asm::SENTINEL_BEGIN_PREFIX
                        ));
                        body.push(crate::inline_asm::INTEL_SYNTAX_WRAPPER.to_string());
                        for (index, chunk) in chunks.iter().enumerate() {
                            body.push(format!("# pinker:sussurro chunk={index}"));
                            let parts = crate::inline_asm::parse_template(chunk)
                                .map_err(|error| err(&error.to_string()))?;
                            let rendered = crate::inline_asm::render_template(&parts, &bindings)
                                .map_err(|error| err(&error.to_string()))?;
                            body.extend(rendered.lines().map(str::to_string));
                        }
                        body.push(crate::inline_asm::ATT_SYNTAX_WRAPPER.to_string());
                        body.push(format!(
                            "{}{envelope_id}",
                            crate::inline_asm::SENTINEL_END_PREFIX
                        ));
                        for operand in operands {
                            if let crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                                name,
                                slot,
                                ty,
                                ..
                            } = operand
                            {
                                let register = bindings[name].att();
                                body.extend(normalize_inline_asm_output(register, *ty)?);
                                body.push(format!(
                                    "movq {}, -{}(%rbp)",
                                    register, slot_offsets[slot]
                                ));
                                body.push(format!(
                                    "# pinker:sussurro output {name} <- {}",
                                    bindings[name].intel()
                                ));
                            }
                        }
                    }
                    SelectedInstr::UnionInject {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        payload_layout,
                        ..
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        let storage = union_storage_offsets
                            .get(&UnionStorageKey {
                                block: block_index,
                                instr: instr_index,
                            })
                            .copied()
                            .ok_or_else(|| {
                                err("storage de união ausente no frame do subset externo montável")
                            })?;
                        // A ABI de criação recebe **endereço**, nunca o payload
                        // reempacotado em `u64`. Escalares e handles são
                        // materializados num scratch do tamanho real; agregados
                        // já são representados por endereço e o próprio
                        // endereço é passado, e o runtime copia imediatamente.
                        match payload_layout.representation {
                            crate::union_payload::UnionPayloadRepresentation::Scalar
                            | crate::union_payload::UnionPayloadRepresentation::OpaqueHandle => {
                                body.extend(load_operand(
                                    "%rax",
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                // O scratch é zerado antes da escrita para que
                                // um payload estreito não vaze bytes anteriores
                                // do frame para dentro do snapshot.
                                body.push(format!("movq $0, -{storage}(%rbp)"));
                                body.extend(store_union_scratch_word(
                                    "%rax",
                                    storage,
                                    payload_layout.size,
                                )?);
                                body.push(format!("leaq -{storage}(%rbp), %r8"));
                            }
                            crate::union_payload::UnionPayloadRepresentation::Aggregate => {
                                body.extend(load_operand(
                                    "%r8",
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                            }
                        }
                        body.push(format!("movq ${}, %rdi", union_type_id.0));
                        body.push(format!("movq ${tag}, %rsi"));
                        body.push(format!("movq ${}, %rdx", payload_layout.size));
                        body.push(format!("movq ${}, %rcx", payload_layout.align));
                        body.push("call pinker_uniao_criar".to_string());
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // A escolha do símbolo interno de ABI acontece **aqui**, no
                    // backend: a AST e a IR não carregam nome de runtime.
                    SelectedInstr::UnionTag {
                        dest,
                        value,
                        union_type_id,
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand("%rdi", value, &slot_offsets, &rodata_strings)?);
                        // A leitura de tag valida também a identidade da união:
                        // um handle de outra união não devolve tag alguma.
                        body.push(format!("movq ${}, %rsi", union_type_id.0));
                        body.push("call pinker_uniao_tag".to_string());
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::UnionExtract {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        payload_layout,
                        ..
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        let storage = union_storage_offsets
                            .get(&UnionStorageKey {
                                block: block_index,
                                instr: instr_index,
                            })
                            .copied()
                            .ok_or_else(|| {
                                err("storage de união ausente no frame do subset externo montável")
                            })?;
                        // A extração copia para storage novo do binding. O
                        // ponteiro interno do descritor nunca é devolvido.
                        body.extend(load_operand("%rdi", value, &slot_offsets, &rodata_strings)?);
                        body.push(format!("movq ${}, %rsi", union_type_id.0));
                        body.push(format!("movq ${tag}, %rdx"));
                        body.push(format!("movq ${}, %rcx", payload_layout.size));
                        body.push(format!("movq ${}, %r8", payload_layout.align));
                        body.push(format!("leaq -{storage}(%rbp), %r9"));
                        body.push("call pinker_uniao_copiar_payload".to_string());
                        match payload_layout.representation {
                            crate::union_payload::UnionPayloadRepresentation::Scalar
                            | crate::union_payload::UnionPayloadRepresentation::OpaqueHandle => {
                                body.extend(load_union_scratch_word(
                                    "%rax",
                                    storage,
                                    payload_layout.size,
                                )?);
                            }
                            crate::union_payload::UnionPayloadRepresentation::Aggregate => {
                                body.push(format!("leaq -{storage}(%rbp), %rax"));
                            }
                        }
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    _ => {
                        return Err(err(
                            "subset externo montável (Fase 135) aceita apenas atribuição, aritmética linear (+,-,*), comparações mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), `virar` mínimo explícito (`u32` slot -> `u64` e `u64` slot -> `u32`), call direta com N argumentos (`bombom`/`u32`/`u64`/`verso` opaco/`seta<T>`; ABI SysV completa, Fase 213/B2), `deref_store` mínimo em `bombom`/`u32`/`u64` (incluindo escrita heterogênea de campo de `ninho` via offset explícito), `deref_load` mínimo em `bombom`/`u32`/`u64` (incluindo campo heterogêneo de `ninho` via offset explícito), literal `verso` estático mínimo em `.rodata` carregado por endereço e tráfego opaco por slot/parâmetro, composição heterogênea mínima auditável no mesmo `ninho` (`u32` + `u64` por offset) e load/store em slots de frame, preservando recorte conservador de `quebrar`/`continuar` em `sempre que` via saltos já materializados (até três níveis de laço aninhado)",
                        ));
                    }
                }
            }
            // @pinker-nav:end backend-s.lowering.falar-runtime
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

    let mut function_refs = std::collections::BTreeSet::new();
    for function in &selected.functions {
        collect_function_refs_in_function(function, &mut function_refs);
    }
    for name in &function_refs {
        if !selected.functions.iter().any(|f| &f.name == name) {
            return Err(err(
                "subset externo montável (Fase 242) encontrou referência a função inexistente como valor",
            ));
        }
    }

    Ok(ExternalCallConvProgram {
        rodata_globals,
        rodata_strings,
        rodata_function_refs: function_refs.into_iter().collect(),
        trait_vtables: trait_vtables.into_values().collect(),
        trait_adapters: trait_adapters.into_values().collect(),
        functions,
    })
}

// Fase 242: varre uma função selecionada coletando todo nome de função
// referenciado como valor callable (`OperandIR::FunctionRef`) em qualquer
// posição de operando — inclusive dentro de `call_indirect`/`call`/`falar`.
fn collect_function_refs_in_function(
    function: &crate::instr_select::SelectedFunction,
    out: &mut std::collections::BTreeSet<String>,
) {
    fn note(op: &OperandIR, out: &mut std::collections::BTreeSet<String>) {
        if let OperandIR::FunctionRef(name) = op {
            out.insert(name.clone());
        }
    }
    for block in &function.blocks {
        for inst in &block.instructions {
            match inst {
                SelectedInstr::Mov { src, .. } => note(src, out),
                SelectedInstr::Neg { operand, .. }
                | SelectedInstr::Not { operand, .. }
                | SelectedInstr::BitNot { operand, .. } => note(operand, out),
                SelectedInstr::DerefLoad { ptr, .. } => note(ptr, out),
                SelectedInstr::DerefStore { ptr, value, .. } => {
                    note(ptr, out);
                    note(value, out);
                }
                SelectedInstr::PointerOffset {
                    pointer, offset, ..
                } => {
                    note(pointer, out);
                    note(offset, out);
                }
                SelectedInstr::Cast { value, .. } => note(value, out),
                SelectedInstr::UnionInject { value, .. }
                | SelectedInstr::UnionTag { value, .. }
                | SelectedInstr::UnionExtract { value, .. } => note(value, out),
                SelectedInstr::BitAnd { lhs, rhs, .. }
                | SelectedInstr::BitOr { lhs, rhs, .. }
                | SelectedInstr::BitXor { lhs, rhs, .. }
                | SelectedInstr::Shl { lhs, rhs, .. }
                | SelectedInstr::Shr { lhs, rhs, .. }
                | SelectedInstr::Add { lhs, rhs, .. }
                | SelectedInstr::Sub { lhs, rhs, .. }
                | SelectedInstr::Mul { lhs, rhs, .. }
                | SelectedInstr::Div { lhs, rhs, .. }
                | SelectedInstr::Mod { lhs, rhs, .. }
                | SelectedInstr::CmpEq { lhs, rhs, .. }
                | SelectedInstr::CmpNe { lhs, rhs, .. }
                | SelectedInstr::CmpLt { lhs, rhs, .. }
                | SelectedInstr::CmpLe { lhs, rhs, .. }
                | SelectedInstr::CmpGt { lhs, rhs, .. }
                | SelectedInstr::CmpGe { lhs, rhs, .. } => {
                    note(lhs, out);
                    note(rhs, out);
                }
                SelectedInstr::Call { args, .. } | SelectedInstr::CallVoid { args, .. } => {
                    for a in args {
                        note(a, out);
                    }
                }
                SelectedInstr::CallIndirect { callee, args, .. } => {
                    note(callee, out);
                    for a in args {
                        note(a, out);
                    }
                }
                SelectedInstr::CallRaw { callee, args, .. } => {
                    note(callee, out);
                    for arg in args {
                        note(arg, out);
                    }
                }
                SelectedInstr::MakeClosure { captures, .. } => {
                    for capture in captures {
                        note(capture, out);
                    }
                }
                SelectedInstr::MakeTraitObject { value, .. } => {
                    note(value, out);
                }
                SelectedInstr::TraitCall { object, args, .. } => {
                    note(object, out);

                    for arg in args {
                        note(arg, out);
                    }
                }
                SelectedInstr::Falar { args } => {
                    for a in args {
                        note(&a.value, out);
                    }
                }
                SelectedInstr::InlineAsm { .. } => {}
            }
        }
        match &block.terminator {
            SelectedTerminator::Ret(Some(v)) => note(v, out),
            SelectedTerminator::Br { cond, .. } => note(cond, out),
            _ => {}
        }
    }
}

// @pinker-nav:start backend-s.renderizacao.callconv-programa
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `render_external_x86_64_linux_callconv_impl` (início): cabeçalho comentado e emissão da seção `.rodata` — globais (ligação decidida por `native_symbol`, hoje `.local`, mais `.type @object`/label/`.quad valor`/`.size`) e strings com layout length-prefixed `[.quad tamanho][.ascii bytes]` (via `escape_gas_string`), seguida da diretiva `.text`. O parâmetro `runtime_init` distingue o caminho nativo do hospedado. Renderer do modelo `ExternalCallConvProgram`, separado do renderer `.s` textual baseado em `BackendTextProgram`.
fn render_external_x86_64_linux_callconv_impl(
    program: &ExternalCallConvProgram,
    runtime_init: bool,
) -> Result<String, PinkerError> {
    let mut out = String::new();
    // R2: o conjunto que este renderer vai emitir é verificado por ele mesmo,
    // antes de o `.s` chegar ao assembler. Colisão conhecível pelo compilador
    // vira diagnóstico Pinker, não `Error: symbol already defined` do GNU as.
    let mut definitions = EmittedDefinitions::new();
    line(
        &mut out,
        0,
        "# pinker v0 external toolchain subset (fase 135, linux x86_64, frame/reg + memoria minima + multiplos blocos/labels + jmp/br + loop minimo + quebrar/continuar camada 3 conservadora (composicao minima ate tres niveis de laço) + globais estaticas minimas em .rodata + composto minimo com deref_store/deref_load heterogeneo camada 4 (composicao `u32`+`u64` no mesmo ninho por offset) + u32/u64 minimos em params/locals + comparacao `>=` minima (camada 4 conservadora de 10.2) + `virar` minimo bidirecional por slot (`u32->u64` e `u64->u32`) + `verso` minimo condicional (literal estatico em .rodata, carga de endereco e trafego opaco em slot/parametro) + abi sysv completa fase 213/B2 (6 regs + args de pilha com padding de alinhamento, N parametros, recursao e chamadas aninhadas))",
    );
    if !program.rodata_globals.is_empty()
        || !program.rodata_strings.is_empty()
        || !program.rodata_function_refs.is_empty()
        || !program.trait_vtables.is_empty()
    {
        line(&mut out, 0, ".section .rodata");
        for global in &program.rodata_globals {
            // F-14: dado global Pinker também captura símbolo do host quando
            // é GLOBAL. `eterno` não atravessa a unidade de link.
            let binding = native_symbol::native_binding(NativeDefinition::UserGlobal);
            definitions.define(&global.name, &format!("eterno {}", global.name));
            line(&mut out, 0, &binding.directive(&global.name));
            line(&mut out, 0, &format!(".type {}, @object", global.name));
            line(&mut out, 0, &format!("{}:", global.name));
            line(&mut out, 1, &format!(".quad {}", global.value));
            line(
                &mut out,
                0,
                &format!(".size {}, .-{}", global.name, global.name),
            );
        }
        for text in &program.rodata_strings {
            // Layout length-prefixed (Fase 215/B4): todo verso — estático ou
            // de heap — é um ponteiro para `[u64 tamanho_em_bytes][bytes]`.
            line(&mut out, 0, ".align 8");
            definitions.define(&text.label, &format!("literal verso #{}", text.label));
            line(&mut out, 0, &format!("{}:", text.label));
            line(&mut out, 1, &format!(".quad {}", text.value.len()));
            if !text.value.is_empty() {
                line(
                    &mut out,
                    1,
                    &format!(".ascii \"{}\"", escape_gas_string(&text.value)),
                );
            }
        }
        for name in &program.rodata_function_refs {
            // Fase 242: descritor callable estático {code_ptr, env_ptr} de
            // função não-capturante. `principal` vira `main` na ABI C, então
            // o descritor aponta para o símbolo renomeado.
            let symbol = native_symbol::function_symbol(NativeSurface::Assemblable, name);
            line(&mut out, 0, ".align 16");
            let descriptor = function_ref_descriptor_label(name);
            definitions.define(&descriptor, &format!("descritor callable de {}", name));
            line(&mut out, 0, &format!("{}:", descriptor));
            line(&mut out, 1, &format!(".quad {}", symbol));
            line(&mut out, 1, ".quad 0");
        }
        for vtable in &program.trait_vtables {
            line(&mut out, 0, ".align 8");
            definitions.define(&vtable.symbol, &format!("vtable {}", vtable.symbol));
            line(&mut out, 0, &format!("{}:", vtable.symbol));
            for entry in &vtable.entries {
                line(&mut out, 1, &format!(".quad {}", entry));
            }
        }
    }
    line(&mut out, 0, ".text");
    let needs_standalone_typed_pointer_offset = !runtime_init
        && program.functions.iter().any(|function| {
            function.blocks.iter().any(|block| {
                block
                    .body
                    .iter()
                    .any(|stmt| stmt == "call pinker_ponteiro_derivar_tipado")
            })
        });
    if needs_standalone_typed_pointer_offset {
        // O subset hospedado continua deliberadamente linkável sem `libpinker_rt`.
        // Esta implementação local conserva os mesmos checks de null e overflow
        // do helper do runtime; layouts inválidos já foram barrados no lowering.
        let helper = "pinker_ponteiro_derivar_tipado";
        definitions.define(helper, "implementação local do subset hospedado");
        line(
            &mut out,
            0,
            &native_symbol::native_binding(NativeDefinition::BackendHelper).directive(helper),
        );
        line(
            &mut out,
            0,
            ".type pinker_ponteiro_derivar_tipado, @function",
        );
        line(&mut out, 0, "pinker_ponteiro_derivar_tipado:");
        line(&mut out, 1, "testq %rdi, %rdi");
        line(&mut out, 1, "jz .Lpinker_ponteiro_derivar_invalido");
        line(&mut out, 1, "movq %rdx, %r8");
        line(&mut out, 1, "movq %rsi, %rax");
        line(&mut out, 1, "mulq %r8");
        line(&mut out, 1, "jo .Lpinker_ponteiro_derivar_invalido");
        line(&mut out, 1, "addq %rdi, %rax");
        line(&mut out, 1, "jc .Lpinker_ponteiro_derivar_invalido");
        line(&mut out, 1, "ret");
        line(&mut out, 0, ".Lpinker_ponteiro_derivar_invalido:");
        line(&mut out, 1, "movq $60, %rax");
        line(&mut out, 1, "movq $1, %rdi");
        line(&mut out, 1, "syscall");
        line(&mut out, 1, "ud2");
        line(
            &mut out,
            0,
            ".size pinker_ponteiro_derivar_tipado, .-pinker_ponteiro_derivar_tipado",
        );
    }
    for adapter in &program.trait_adapters {
        definitions.define(
            &adapter.symbol,
            &format!("adapter de trato -> {}", adapter.target),
        );
        line(
            &mut out,
            0,
            &native_symbol::native_binding(NativeDefinition::BackendHelper)
                .directive(&adapter.symbol),
        );
        line(&mut out, 0, &format!(".type {}, @function", adapter.symbol));
        line(&mut out, 0, &format!("{}:", adapter.symbol));
        line(
            &mut out,
            1,
            trait_adapter_receiver_load(adapter.concrete_type),
        );
        line(&mut out, 1, &format!("jmp {}", adapter.target));
        line(
            &mut out,
            0,
            &format!(".size {}, .-{}", adapter.symbol, adapter.symbol),
        );
    }
    // @pinker-nav:end backend-s.renderizacao.callconv-programa

    // @pinker-nav:start backend-s.abi.prologo-parametros
    // @pinker-nav:domain abi
    // @pinker-nav:layer backend-s
    // @pinker-nav:summary Prólogo e passagem de parâmetros do renderer montável: pede o símbolo e a ligação à autoridade única `native_symbol` (`principal` produz `main`, GLOBAL; toda outra definição é `.local`), emite a diretiva de ligação e `.type @function`, depois `pushq %rbp`/`movq %rsp,%rbp`, insere a chamada a `pinker_rt_iniciar` quando `runtime_init` e a identidade é a do entrypoint (pilha alinhada a 16 após o push), reserva o frame (`subq $stack_size,%rsp`), armazena os 6 primeiros parâmetros de `ARG_REGS` nos slots e carrega o 7º+ a partir de `16(%rbp)`. A única diferença observável entre os dois caminhos externos é essa chamada de runtime.
    for function in &program.functions {
        let symbol = native_symbol::function_symbol(NativeSurface::Assemblable, &function.name);
        let binding = native_symbol::function_binding(&function.name);
        definitions.define(&symbol, &function.name);
        line(&mut out, 0, &binding.directive(&symbol));
        line(&mut out, 0, &format!(".type {}, @function", symbol));
        line(&mut out, 0, &format!("{}:", symbol));
        line(&mut out, 1, "pushq %rbp");
        line(&mut out, 1, "movq %rsp, %rbp");
        if runtime_init && native_symbol::is_entrypoint(&function.name) {
            // argc em %rdi e argv em %rsi (ABI C do main); pilha alinhada a 16
            // após o push do %rbp, então a chamada é válida aqui.
            line(&mut out, 1, "call pinker_rt_iniciar");
        }
        if function.stack_size > 0 {
            line(&mut out, 1, &format!("subq ${}, %rsp", function.stack_size));
        }
        for (idx, param) in function.params.iter().enumerate() {
            if idx < ARG_REGS.len() {
                line(
                    &mut out,
                    1,
                    &format!(
                        "movq {}, -{}(%rbp)",
                        ARG_REGS[idx], function.slot_offsets[param]
                    ),
                );
            } else {
                // Parâmetros 7+ chegam pela pilha do chamador: retorno em
                // 8(%rbp), primeiro argumento de pilha em 16(%rbp).
                let incoming = 16 + 8 * (idx - ARG_REGS.len());
                line(&mut out, 1, &format!("movq {}(%rbp), %r10", incoming));
                line(
                    &mut out,
                    1,
                    &format!("movq %r10, -{}(%rbp)", function.slot_offsets[param]),
                );
            }
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
        // @pinker-nav:end backend-s.abi.prologo-parametros

        // @pinker-nav:start backend-s.abi.blocos-terminadores
        // @pinker-nav:domain abi
        // @pinker-nav:layer backend-s
        // @pinker-nav:summary Emissão de blocos e terminadores do renderer montável: rótulos locais injetivos de `native_symbol::injective_local_label` (`.Lp<len>_<fn><len>_<bloco>`, prefixados por comprimento e portanto recuperáveis), corpo já textualizado linha a linha, e cada terminador — `Jmp` → `jmp`; `Br` carrega a condição (`load_operand` com `.expect`), `cmpq $0,%rax` + `jne`/`jmp`; `Ret` carrega o valor (`.expect`), `leave` e `ret`. Fecha a função com `.size`, exceto quando ela carrega envelope de `sussurro` — declarar o tamanho ali tornaria falso o invariante de artefato de D4, que compara o objeto real contra a baseline sem os envelopes. No fim, verifica o conjunto emitido: duas identidades distintas no mesmo símbolo viram diagnóstico Pinker determinístico em vez de erro cru do GNU as. Os `.expect` dependem de invariantes garantidas antes, no lowering (condição/retorno carregáveis). Encerra a unidade declarando `.section .note.GNU-stack,"",@progbits` uma única vez, depois de todas as seções executáveis e de dados: a unidade informa que não exige pilha executável, sem o que o assembler marca esse requisito por omissão e o linker o propaga para `PT_GNU_STACK` do executável final. A declaração pertence à unidade e não à função, e não toca rótulo, símbolo, `.size`, CFI ou alinhamento.
        line(
            &mut out,
            1,
            &format!(
                "jmp {}",
                native_symbol::injective_local_label(&[&function.name, "entry"])
            ),
        );
        for block in &function.blocks {
            let block_label = native_symbol::injective_local_label(&[&function.name, &block.label]);
            definitions.define(
                &block_label,
                &format!("bloco {} de {}", block.label, function.name),
            );
            line(&mut out, 0, &format!("{}:", block_label));
            for stmt in &block.body {
                if !runtime_init && stmt.starts_with("call pinker_publico_validar_") {
                    continue;
                }
                line(&mut out, 1, stmt);
            }
            match &block.terminator {
                ExternalCallConvTerminator::Jmp(target) => {
                    line(
                        &mut out,
                        1,
                        &format!(
                            "jmp {}",
                            native_symbol::injective_local_label(&[&function.name, target])
                        ),
                    );
                }
                ExternalCallConvTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => {
                    for stmt in load_operand(
                        REG_RET,
                        cond,
                        &function.slot_offsets,
                        &program.rodata_strings,
                    )
                    .expect("condição do branch deve ser carregável")
                    {
                        line(&mut out, 1, &stmt);
                    }
                    line(&mut out, 1, "cmpq $0, %rax");
                    line(
                        &mut out,
                        1,
                        &format!(
                            "jne {}",
                            native_symbol::injective_local_label(&[&function.name, then_label])
                        ),
                    );
                    line(
                        &mut out,
                        1,
                        &format!(
                            "jmp {}",
                            native_symbol::injective_local_label(&[&function.name, else_label])
                        ),
                    );
                }
                ExternalCallConvTerminator::Ret(value) => {
                    for stmt in load_operand(
                        REG_RET,
                        value,
                        &function.slot_offsets,
                        &program.rodata_strings,
                    )
                    .expect("retorno deve ser carregável")
                    {
                        line(&mut out, 1, &stmt);
                    }
                    line(&mut out, 1, "leave");
                    line(&mut out, 1, "ret");
                }
                ExternalCallConvTerminator::RetVoid => {
                    line(&mut out, 1, "leave");
                    line(&mut out, 1, "ret");
                }
            }
        }
        // F-09: `.size` só é omitido na única classe em que o próprio produto
        // proíbe declará-lo — função que carrega envelope de `sussurro`. O
        // invariante de artefato de D4 compara o objeto real contra a baseline
        // sem os envelopes, e o envelope muda o tamanho da função por
        // construção. Declarar o tamanho aqui abortaria todo build com
        // `sussurro`; inventar um tamanho seria pior. D4 não é reaberta nesta
        // Task.
        if !function_carries_inline_asm_envelope(function) {
            line(&mut out, 0, &format!(".size {}, .-{}", symbol, symbol));
        }
    }
    if let Some(collision) = definitions.first_collision() {
        return Err(err(&native_symbol::emitted_collision_message(collision)));
    }
    // F-10: a unidade declara explicitamente que não exige pilha executável.
    // Sem esta nota o `as` marca o objeto como "requer stack executável" por
    // omissão e o linker propaga isso para `PT_GNU_STACK = RWE` no executável
    // final, mesmo com todos os membros de `libpinker_rt.a` já compatíveis.
    // A declaração pertence à unidade, não à função: é emitida uma única vez,
    // depois de todas as seções executáveis e de dados, e não toca label,
    // símbolo, `.size`, CFI ou alinhamento. Este renderer é o único caminho
    // montável do backend e é sempre ELF/GNU (x86-64 Linux SysV), então a
    // diretiva não é condicional — o renderer `.s` textual não passa por aqui.
    line(&mut out, 0, ".section .note.GNU-stack,\"\",@progbits");
    Ok(out)
}

/// A função carrega um envelope de `sussurro`?
///
/// Só é consultado para decidir a emissão de `.size`; nenhuma outra decisão de
/// renderização depende da presença do envelope.
fn function_carries_inline_asm_envelope(function: &ExternalCallConvFunction) -> bool {
    function.blocks.iter().any(|block| {
        block.body.iter().any(|stmt| {
            stmt.trim_start()
                .starts_with(crate::inline_asm::SENTINEL_BEGIN_PREFIX)
        })
    })
}
// @pinker-nav:end backend-s.abi.blocos-terminadores

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

// @pinker-nav:start backend-s.lowering.operacoes-lineares
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-s
// @pinker-nav:summary Helpers de lowering de operações lineares e comparações: `lower_linear_binop` (carrega `lhs`/`rhs`, aplica `addq`/`subq`/`imulq` e guarda no slot) e os seis `lower_cmp_eq`/`_ne`/`_lt`/`_gt`/`_le`/`_ge` (`cmpq` + `set*` + `movzbq`). Usam `%rax`/`%r10`, registram strings de `.rodata` dos operandos e materializam o resultado no slot do temporário. As comparações `<`/`>`/`<=`/`>=` usam `setb`/`seta`/`setbe`/`setae` (unsigned), sem distinção de signedness. Mantidas em uma única região contígua por serem variações do mesmo padrão.
fn lower_linear_binop(
    opcode: &str,
    dest: crate::cfg_ir::TempIR,
    operands: (&OperandIR, &OperandIR),
    ty: TypeIR,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let (lhs, rhs) = operands;
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("{} {}, {}", opcode, REG_TMP, REG_RET));
    body.extend(normalize_rax(ty));
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn normalize_rax(ty: TypeIR) -> Vec<String> {
    let instruction = match ty {
        TypeIR::U8 => Some("movzbq %al, %rax"),
        TypeIR::I8 => Some("movsbq %al, %rax"),
        TypeIR::U16 => Some("movzwq %ax, %rax"),
        TypeIR::I16 => Some("movswq %ax, %rax"),
        TypeIR::U32 => Some("movl %eax, %eax"),
        TypeIR::I32 => Some("movslq %eax, %rax"),
        _ => None,
    };
    instruction.into_iter().map(str::to_string).collect()
}

#[allow(clippy::too_many_arguments)]
fn lower_shift(
    right: bool,
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    ty: TypeIR,
    function_name: &str,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let width = match ty {
        TypeIR::U8 | TypeIR::I8 => 8,
        TypeIR::U16 | TypeIR::I16 => 16,
        TypeIR::U32 | TypeIR::I32 => 32,
        _ => 64,
    };
    let valid_label =
        native_symbol::injective_local_label(&[function_name, "shift_valid", &dest.0.to_string()]);
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("cmpq ${width}, {REG_TMP}"));
    body.push(format!("jb {valid_label}"));
    body.push(format!("movq {REG_TMP}, %rdi"));
    body.push(format!("movq ${width}, %rsi"));
    body.push("call pinker_erro_shift_count".to_string());
    body.push(format!("{valid_label}:"));
    body.push("movb %r10b, %cl".to_string());
    let opcode = if right {
        if ty.is_signed() {
            "sarq"
        } else {
            "shrq"
        }
    } else {
        "shlq"
    };
    body.push(format!("{opcode} %cl, {REG_RET}"));
    body.extend(normalize_rax(ty));
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

#[allow(clippy::too_many_arguments)]
fn lower_div_mod(
    modulo: bool,
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    ty: TypeIR,
    function_name: &str,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let temp = dest.0.to_string();
    let nonzero = native_symbol::injective_local_label(&[function_name, "div_nonzero", &temp]);
    let regular = native_symbol::injective_local_label(&[function_name, "div_regular", &temp]);
    let done = native_symbol::injective_local_label(&[function_name, "div_done", &temp]);
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("testq {REG_TMP}, {REG_TMP}"));
    body.push(format!("jne {nonzero}"));
    body.push("call pinker_erro_divisao_zero".to_string());
    body.push(format!("{nonzero}:"));
    if ty.is_signed() {
        let min = match ty {
            TypeIR::I8 => i8::MIN as i64,
            TypeIR::I16 => i16::MIN as i64,
            TypeIR::I32 => i32::MIN as i64,
            _ => i64::MIN,
        };
        body.push(format!("cmpq $-1, {REG_TMP}"));
        body.push(format!("jne {regular}"));
        body.push(format!("movabsq ${min}, %rdx"));
        body.push("cmpq %rdx, %rax".to_string());
        body.push(format!("jne {regular}"));
        if modulo {
            body.push("xorq %rax, %rax".to_string());
        }
        body.push(format!("jmp {done}"));
        body.push(format!("{regular}:"));
        body.push("cqto".to_string());
        body.push(format!("idivq {REG_TMP}"));
    } else {
        body.push("xorq %rdx, %rdx".to_string());
        body.push(format!("divq {REG_TMP}"));
    }
    if modulo {
        body.push("movq %rdx, %rax".to_string());
    }
    body.push(format!("{done}:"));
    body.extend(normalize_rax(ty));
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
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
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
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
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

fn selected_comparison_is_signed(
    function: &crate::instr_select::SelectedFunction,
    lhs: &OperandIR,
    rhs: &OperandIR,
) -> bool {
    let mut visiting = HashSet::new();
    if selected_operand_is_signed(function, lhs, &mut visiting) {
        return true;
    }
    visiting.clear();
    selected_operand_is_signed(function, rhs, &mut visiting)
}

fn lower_public_pointer_derivation(
    function: &crate::instr_select::SelectedFunction,
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    is_subtraction: bool,
    slot_offsets: &HashMap<String, u32>,
    rodata_strings: &[ExternalCallConvString],
) -> Result<Vec<String>, PinkerError> {
    let mut visiting_temps = HashSet::new();
    let mut visiting_slots = HashSet::new();
    let lhs_is_pointer =
        selected_operand_is_public_pointer(function, lhs, &mut visiting_temps, &mut visiting_slots);
    visiting_temps.clear();
    visiting_slots.clear();
    let rhs_is_pointer =
        selected_operand_is_public_pointer(function, rhs, &mut visiting_temps, &mut visiting_slots);
    let origin = if is_subtraction {
        (lhs_is_pointer && !rhs_is_pointer).then_some(lhs)
    } else {
        match (lhs_is_pointer, rhs_is_pointer) {
            (true, false) => Some(lhs),
            (false, true) => Some(rhs),
            _ => None,
        }
    };
    let Some(origin) = origin else {
        return Ok(Vec::new());
    };
    let mut body = load_operand("%rdi", origin, slot_offsets, rodata_strings)?;
    body.push(format!(
        "movq -{}(%rbp), %rsi",
        slot_offsets[&temp_key(dest)]
    ));
    body.push("call pinker_publico_validar_derivacao".to_string());
    Ok(body)
}

fn lower_typed_pointer_offset(
    function: &crate::instr_select::SelectedFunction,
    dest: crate::cfg_ir::TempIR,
    pointer: &OperandIR,
    offset: &OperandIR,
    element_layout: (u64, u64),
    slot_offsets: &HashMap<String, u32>,
    rodata_strings: &[ExternalCallConvString],
) -> Result<Vec<String>, PinkerError> {
    let (element_size, element_align) = element_layout;
    let mut body = load_operand("%rdi", pointer, slot_offsets, rodata_strings)?;
    body.extend(load_operand("%rsi", offset, slot_offsets, rodata_strings)?);
    body.push(format!("movq ${}, %rdx", element_size));
    body.push(format!("movq ${}, %rcx", element_align));
    body.push("call pinker_ponteiro_derivar_tipado".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));

    let mut visiting_temps = HashSet::new();
    let mut visiting_slots = HashSet::new();
    if selected_operand_provenance(function, pointer, &mut visiting_temps, &mut visiting_slots)
        .requires_access_check()
    {
        body.extend(load_operand("%rdi", pointer, slot_offsets, rodata_strings)?);
        body.push(format!(
            "movq -{}(%rbp), %rsi",
            slot_offsets[&temp_key(dest)]
        ));
        body.push("call pinker_publico_validar_derivacao".to_string());
    }
    Ok(body)
}

/// Proveniência de um ponteiro, do ponto de vista do validador de acesso do
/// runtime nativo.
///
/// A classificação decide se `deref_load`/`deref_store` passam por
/// `pinker_publico_validar_acesso`. Antes do hotfix pós-PR #411 ela era um
/// booleano "é público?", e converter um inteiro em ponteiro caía no ramo
/// falso: o acesso era emitido cru e o processo morria por SIGSEGV, enquanto o
/// interpretador diagnosticava o mesmo programa.
///
/// São **quatro** classes, e as duas últimas não são a mesma coisa:
/// `Fabricated` afirma que a origem é um valor não-ponteiro; `Unclassified`
/// afirma apenas que a origem não foi determinada. Confundir as duas foi o
/// defeito apontado na revisão do head `3725118` — um cast entre tipos de
/// ponteiro promovia `Unclassified` a `Fabricated` só por falta de informação.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PointerProvenance {
    /// Domínio interno do runtime — hoje, o ambiente de closure recebido como
    /// parâmetro. Não é memória pública e não pode ser confrontado com o
    /// registro público: validar aqui rejeitaria um acesso legítimo.
    Internal,
    /// Região pública conhecida: resultado de `alocar`, de uma chamada que
    /// devolve ponteiro — em **qualquer** forma de chamada, ver
    /// `selected_call_provenance` —, de um parâmetro de ponteiro, ou derivação
    /// dessas.
    Public,
    /// Endereço construído a partir de um valor **não-ponteiro**, tipicamente
    /// um inteiro (`<inteiro> virar seta<T>`). Não há proveniência para
    /// rastrear; o acesso precisa ser validado para que o runtime recuse
    /// endereço nunca registrado em vez de escrever em memória real.
    Fabricated,
    /// Ponteiro cuja origem não foi determinada pela análise atual — por
    /// exemplo, carregado de memória.
    ///
    /// Não é sinônimo de inteiro, e não recebe a garantia de falha controlada:
    /// é um limite conhecido, documentado em `MANUAL.md`. Fechar a classe exige
    /// análise de domínio com contrato próprio; tratá-la como exigente foi
    /// testado e rejeita acesso legítimo de closure.
    Unclassified,
}

impl PointerProvenance {
    /// Um acesso é validado quando o ponteiro pertence à memória pública ou
    /// quando foi fabricado. `Internal` é isenta porque tem domínio próprio, e
    /// `Unclassified` fica de fora como limite reconhecido — não como
    /// afirmação de que aquele acesso é seguro.
    fn requires_access_check(self) -> bool {
        matches!(self, Self::Public | Self::Fabricated)
    }

    /// Combina as proveniências de várias atribuições ao mesmo destino: vence a
    /// classe mais exigente, porque o acesso precisa ser seguro para qualquer
    /// caminho que chegue até ele.
    fn join(self, other: Self) -> Self {
        for classe in [Self::Fabricated, Self::Public, Self::Internal] {
            if self == classe || other == classe {
                return classe;
            }
        }
        Self::Unclassified
    }
}

fn selected_operand_is_public_pointer(
    function: &crate::instr_select::SelectedFunction,
    operand: &OperandIR,
    visiting_temps: &mut HashSet<crate::cfg_ir::TempIR>,
    visiting_slots: &mut HashSet<String>,
) -> bool {
    selected_operand_provenance(function, operand, visiting_temps, visiting_slots)
        .requires_access_check()
}

/// Forma de chamada selecionada, reduzida ao que a proveniência precisa saber.
///
/// `dest` é `None` para a chamada sem valor de retorno.
struct SelectedCallShape<'a> {
    dest: Option<crate::cfg_ir::TempIR>,
    /// Símbolo chamado, quando a chamada é por nome. `None` nas formas
    /// indiretas — chamada por valor callable, por endereço cru de código ou
    /// por slot de vtable.
    callee: Option<&'a str>,
    ret_type: TypeIR,
}

/// **Autoridade única** sobre as formas de chamada do IR selecionado.
///
/// Toda regra que dependa de "isto é uma chamada, e o que ela devolve" passa por
/// aqui, para que nenhuma forma nova entre no back-end classificada por engano
/// em um braço isolado. `CallVoid` fica de fora de propósito: não produz valor,
/// logo não produz proveniência nem tipo de destino.
fn selected_call_shape(instruction: &SelectedInstr) -> Option<SelectedCallShape<'_>> {
    match instruction {
        SelectedInstr::Call {
            dest,
            callee,
            ret_type,
            ..
        } => Some(SelectedCallShape {
            dest: Some(*dest),
            callee: Some(callee.as_str()),
            ret_type: *ret_type,
        }),
        SelectedInstr::CallIndirect { dest, ret_type, .. } => Some(SelectedCallShape {
            dest: Some(*dest),
            callee: None,
            ret_type: *ret_type,
        }),
        SelectedInstr::CallRaw { dest, ret_type, .. }
        | SelectedInstr::TraitCall { dest, ret_type, .. } => Some(SelectedCallShape {
            dest: *dest,
            callee: None,
            ret_type: *ret_type,
        }),
        _ => None,
    }
}

/// Proveniência do valor devolvido por uma chamada, qualquer que seja a forma.
///
/// O que decide é o **tipo de retorno**, não a forma do alvo: uma função que
/// devolve `seta<T>` devolve ponteiro de região pública tanto chamada por
/// símbolo quanto por valor callable, por endereço cru de código ou por slot de
/// vtable. Classificar só a forma direta como `Public` deixava o resultado das
/// formas indiretas fora da validação, e o acesso após `liberar` descia cru:
/// o processo morria por SIGSEGV em vez de diagnosticar
/// `E-RUNTIME-MEM-USE-AFTER-FREE`.
///
/// Retorno que não é ponteiro nunca vira `Public`.
fn selected_call_provenance(
    instruction: &SelectedInstr,
    temp: crate::cfg_ir::TempIR,
) -> Option<PointerProvenance> {
    let shape = selected_call_shape(instruction)?;
    if shape.dest != Some(temp) {
        return None;
    }
    Some(
        if shape.callee == Some("alocar") || matches!(shape.ret_type, TypeIR::Pointer { .. }) {
            PointerProvenance::Public
        } else {
            PointerProvenance::Unclassified
        },
    )
}

/// O operando já é **tipado** como ponteiro no ponto da seleção?
///
/// Distingue "a análise não sabe de onde veio" de "não é ponteiro". Só a
/// segunda situação fabrica endereço num `virar seta<T>`; a primeira apenas
/// troca o tipo apontado.
///
/// A autoridade de tipos é a que o pipeline já transporta — `slot_types` para
/// locais e parâmetros, e o tipo que cada instrução selecionada carrega no seu
/// próprio destino. Não existe tabela paralela, e nada aqui olha nome de slot,
/// texto emitido ou qualquer outra heurística: ausência de informação responde
/// `false`, que é o lado conservador (o acesso passa a ser validado).
fn selected_operand_is_pointer_typed(
    function: &crate::instr_select::SelectedFunction,
    operand: &OperandIR,
) -> bool {
    matches!(
        selected_operand_type(function, operand),
        Some(TypeIR::Pointer { .. })
    )
}

fn selected_operand_type(
    function: &crate::instr_select::SelectedFunction,
    operand: &OperandIR,
) -> Option<TypeIR> {
    match operand {
        OperandIR::Local(slot) => function.slot_types.get(slot).copied(),
        OperandIR::Temp(temp) => selected_temp_type(function, *temp),
        // Literais e referências de símbolo não são ponteiro de dados: um
        // `virar seta<T>` sobre eles fabrica endereço.
        _ => None,
    }
}

/// Tipo do temporário, lido da instrução que o define.
///
/// Cada variante devolve o tipo do **destino**, não o tipo dos operandos: por
/// isso as comparações respondem `Logica` mesmo quando comparam ponteiros.
/// Variantes cujo destino não tem tipo transportado respondem `None`.
fn selected_temp_type(
    function: &crate::instr_select::SelectedFunction,
    temp: crate::cfg_ir::TempIR,
) -> Option<TypeIR> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| {
            if let Some(shape) = selected_call_shape(instruction) {
                return (shape.dest == Some(temp)).then_some(shape.ret_type);
            }
            match instruction {
                SelectedInstr::Cast {
                    dest, target_type, ..
                } if *dest == temp => Some(*target_type),
                SelectedInstr::DerefLoad { dest, ty, .. }
                | SelectedInstr::Neg { dest, ty, .. }
                | SelectedInstr::BitNot { dest, ty, .. }
                | SelectedInstr::BitAnd { dest, ty, .. }
                | SelectedInstr::BitOr { dest, ty, .. }
                | SelectedInstr::BitXor { dest, ty, .. }
                | SelectedInstr::Shl { dest, ty, .. }
                | SelectedInstr::Shr { dest, ty, .. }
                | SelectedInstr::Add { dest, ty, .. }
                | SelectedInstr::Sub { dest, ty, .. }
                | SelectedInstr::Mul { dest, ty, .. }
                | SelectedInstr::Div { dest, ty, .. }
                | SelectedInstr::Mod { dest, ty, .. }
                    if *dest == temp =>
                {
                    Some(*ty)
                }
                SelectedInstr::PointerOffset {
                    dest, pointer_type, ..
                } if *dest == temp => Some(*pointer_type),
                // Destino lógico: `ty` descreve os operandos comparados, nunca o
                // resultado. Confundir os dois faria uma comparação de ponteiros
                // parecer um ponteiro.
                SelectedInstr::Not { dest, .. }
                | SelectedInstr::CmpEq { dest, .. }
                | SelectedInstr::CmpNe { dest, .. }
                | SelectedInstr::CmpLt { dest, .. }
                | SelectedInstr::CmpLe { dest, .. }
                | SelectedInstr::CmpGt { dest, .. }
                | SelectedInstr::CmpGe { dest, .. }
                    if *dest == temp =>
                {
                    Some(TypeIR::Logica)
                }
                SelectedInstr::UnionInject {
                    dest,
                    union_type_id,
                    ..
                } if *dest == temp => Some(TypeIR::Union(*union_type_id)),
                SelectedInstr::UnionExtract {
                    dest, payload_type, ..
                } if *dest == temp => Some(*payload_type),
                SelectedInstr::UnionTag { dest, .. } if *dest == temp => Some(TypeIR::U64),
                SelectedInstr::MakeClosure { dest, .. } if *dest == temp => Some(TypeIR::Function),
                SelectedInstr::MakeTraitObject { dest, .. } if *dest == temp => {
                    Some(TypeIR::TraitObject)
                }
                _ => None,
            }
        })
}

fn selected_operand_provenance(
    function: &crate::instr_select::SelectedFunction,
    operand: &OperandIR,
    visiting_temps: &mut HashSet<crate::cfg_ir::TempIR>,
    visiting_slots: &mut HashSet<String>,
) -> PointerProvenance {
    match operand {
        OperandIR::Local(slot) => {
            if function.internal_pointer_params.contains(slot) {
                return PointerProvenance::Internal;
            }
            let has_assignment = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(instruction, SelectedInstr::Mov { dest, .. } if dest == slot)
                });
            if !has_assignment
                && function
                    .slot_types
                    .get(slot)
                    .is_some_and(|ty| matches!(ty, TypeIR::Pointer { .. }))
            {
                return PointerProvenance::Public;
            }
            if !visiting_slots.insert(slot.clone()) {
                return PointerProvenance::Unclassified;
            }
            let provenance = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction {
                    SelectedInstr::Mov { dest, src } if dest == slot => Some(src),
                    _ => None,
                })
                .fold(PointerProvenance::Unclassified, |acumulado, src| {
                    acumulado.join(selected_operand_provenance(
                        function,
                        src,
                        visiting_temps,
                        visiting_slots,
                    ))
                });
            visiting_slots.remove(slot);
            provenance
        }
        OperandIR::Temp(temp) => {
            selected_temp_provenance(function, *temp, visiting_temps, visiting_slots)
        }
        _ => PointerProvenance::Unclassified,
    }
}

fn selected_temp_provenance(
    function: &crate::instr_select::SelectedFunction,
    temp: crate::cfg_ir::TempIR,
    visiting_temps: &mut HashSet<crate::cfg_ir::TempIR>,
    visiting_slots: &mut HashSet<String>,
) -> PointerProvenance {
    if !visiting_temps.insert(temp) {
        return PointerProvenance::Unclassified;
    }
    let provenance = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| {
            if let Some(provenance) = selected_call_provenance(instruction, temp) {
                return Some(provenance);
            }
            match instruction {
                SelectedInstr::Cast {
                    dest,
                    value,
                    target_type,
                } if *dest == temp && matches!(target_type, TypeIR::Pointer { .. }) => {
                    let origem = selected_operand_provenance(
                        function,
                        value,
                        visiting_temps,
                        visiting_slots,
                    );
                    Some(match origem {
                        // A origem já é um ponteiro rastreável: `virar` só muda o
                        // tipo apontado e preserva a proveniência.
                        PointerProvenance::Internal
                        | PointerProvenance::Public
                        | PointerProvenance::Fabricated => origem,
                        // Proveniência desconhecida não é o mesmo que origem
                        // inteira. Quem decide é o **tipo operacional** da origem:
                        // ponteiro→ponteiro só troca o tipo apontado e preserva a
                        // classe; só a conversão de um valor não-ponteiro
                        // (tipicamente um inteiro) fabrica um endereço.
                        PointerProvenance::Unclassified => {
                            if selected_operand_is_pointer_typed(function, value) {
                                PointerProvenance::Unclassified
                            } else {
                                PointerProvenance::Fabricated
                            }
                        }
                    })
                }
                SelectedInstr::Add { dest, lhs, rhs, .. } if *dest == temp => Some(
                    selected_operand_provenance(function, lhs, visiting_temps, visiting_slots)
                        .join(selected_operand_provenance(
                            function,
                            rhs,
                            visiting_temps,
                            visiting_slots,
                        )),
                ),
                SelectedInstr::PointerOffset { dest, pointer, .. } if *dest == temp => Some(
                    selected_operand_provenance(function, pointer, visiting_temps, visiting_slots),
                ),
                SelectedInstr::Sub { dest, lhs, .. } if *dest == temp => Some(
                    selected_operand_provenance(function, lhs, visiting_temps, visiting_slots),
                ),
                _ => None,
            }
        })
        .unwrap_or(PointerProvenance::Unclassified);
    visiting_temps.remove(&temp);
    provenance
}

fn selected_operand_is_signed(
    function: &crate::instr_select::SelectedFunction,
    operand: &OperandIR,
    visiting: &mut HashSet<crate::cfg_ir::TempIR>,
) -> bool {
    match operand {
        OperandIR::Local(slot) => function.slot_types.get(slot).is_some_and(TypeIR::is_signed),
        OperandIR::Temp(temp) => selected_temp_is_signed(function, *temp, visiting),
        _ => false,
    }
}

fn selected_temp_is_signed(
    function: &crate::instr_select::SelectedFunction,
    temp: crate::cfg_ir::TempIR,
    visiting: &mut HashSet<crate::cfg_ir::TempIR>,
) -> bool {
    if !visiting.insert(temp) {
        return false;
    }
    let signed = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            SelectedInstr::Neg { dest, operand, .. } if *dest == temp => {
                Some(selected_operand_is_signed(function, operand, visiting))
            }
            SelectedInstr::DerefLoad { dest, ty, .. } if *dest == temp => Some(ty.is_signed()),
            SelectedInstr::Cast {
                dest, target_type, ..
            } if *dest == temp => Some(target_type.is_signed()),
            SelectedInstr::BitAnd { dest, lhs, rhs, .. }
            | SelectedInstr::BitOr { dest, lhs, rhs, .. }
            | SelectedInstr::BitXor { dest, lhs, rhs, .. }
            | SelectedInstr::Shl { dest, lhs, rhs, .. }
            | SelectedInstr::Shr { dest, lhs, rhs, .. }
            | SelectedInstr::Add { dest, lhs, rhs, .. }
            | SelectedInstr::Sub { dest, lhs, rhs, .. }
            | SelectedInstr::Mul { dest, lhs, rhs, .. }
            | SelectedInstr::Div { dest, lhs, rhs, .. }
            | SelectedInstr::Mod { dest, lhs, rhs, .. }
                if *dest == temp =>
            {
                Some(
                    selected_operand_is_signed(function, lhs, visiting)
                        || selected_operand_is_signed(function, rhs, visiting),
                )
            }
            SelectedInstr::Call { dest, ret_type, .. }
            | SelectedInstr::CallIndirect { dest, ret_type, .. }
                if *dest == temp =>
            {
                Some(ret_type.is_signed())
            }
            SelectedInstr::CallRaw {
                dest: Some(dest),
                ret_type,
                ..
            }
            | SelectedInstr::TraitCall {
                dest: Some(dest),
                ret_type,
                ..
            } if *dest == temp => Some(ret_type.is_signed()),
            _ => None,
        })
        .unwrap_or(false);
    visiting.remove(&temp);
    signed
}

fn lower_cmp_lt(
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    signed: bool,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push(format!("{} %al", if signed { "setl" } else { "setb" }));
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
    signed: bool,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push(format!("{} %al", if signed { "setg" } else { "seta" }));
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
    signed: bool,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push(format!("{} %al", if signed { "setle" } else { "setbe" }));
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
    signed: bool,
    slot_offsets: &HashMap<String, u32>,
    rodata_string_labels: &mut HashMap<String, String>,
    rodata_strings: &mut Vec<ExternalCallConvString>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    register_rodata_strings_for_operand(lhs, rodata_string_labels, rodata_strings);
    register_rodata_strings_for_operand(rhs, rodata_string_labels, rodata_strings);
    body.extend(load_operand(REG_RET, lhs, slot_offsets, rodata_strings)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets, rodata_strings)?);
    body.push(format!("cmpq {}, {}", REG_TMP, REG_RET));
    body.push(format!("{} %al", if signed { "setge" } else { "setae" }));
    body.push("movzbq %al, %rax".to_string());
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}
// @pinker-nav:end backend-s.lowering.operacoes-lineares

// @pinker-nav:start backend-s.lowering.operandos-slots
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-s
// @pinker-nav:summary Coleta de temporários, carga de operandos e nomeação de slots: `collect_temp_ids` (varre instruções e retornos para reunir os `%tN` que ocupam slots de frame), `load_operand` (carrega `Int`/`Bool` via `movabsq`, `Local`/`Temp` de `-off(%rbp)`, `GlobalConst` RIP-relative, `Str` por `leaq label(%rip)` do rodata materializado) e `temp_key` (nome canônico `%tN`). Alimentam o cálculo de frame e a emissão de acesso a slots.
/// Identifica, de forma determinística, o storage de frame de uma operação de
/// união dentro de uma função.
///
/// A chave é posicional porque cada `union_inject`/`union_extract` precisa de
/// storage próprio: reaproveitar storage entre instruções faria duas extrações
/// compartilharem memória, quebrando a independência exigida por HR3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UnionStorageKey {
    block: usize,
    instr: usize,
}

/// Grava a largura **real** do payload escalar no scratch.
///
/// Um `u8` grava um byte; um `u32`, quatro. Gravar sempre oito bytes copiaria
/// lixo do frame para dentro do snapshot imutável.
fn store_union_scratch_word(reg: &str, offset: u32, size: u64) -> Result<Vec<String>, PinkerError> {
    let byte_reg = match reg {
        "%rax" => "%al",
        _ => return Err(err("registrador não suportado no scratch de união")),
    };
    let word_reg = match reg {
        "%rax" => "%ax",
        _ => return Err(err("registrador não suportado no scratch de união")),
    };
    let long_reg = match reg {
        "%rax" => "%eax",
        _ => return Err(err("registrador não suportado no scratch de união")),
    };
    Ok(match size {
        1 => vec![format!("movb {byte_reg}, -{offset}(%rbp)")],
        2 => vec![format!("movw {word_reg}, -{offset}(%rbp)")],
        4 => vec![format!("movl {long_reg}, -{offset}(%rbp)")],
        8 => vec![format!("movq {reg}, -{offset}(%rbp)")],
        _ => {
            return Err(err(
                "subset externo montável só materializa payload escalar de união com 1, 2, 4 ou 8 \
                 bytes",
            ));
        }
    })
}

/// Lê a largura real do payload escalar do storage de extração.
///
/// A carga é sempre estendida com zero para a palavra: a normalização com sinal
/// dos inteiros assinados continua sendo responsabilidade dos consumidores, que
/// já a fazem pelo `TypeIR`.
fn load_union_scratch_word(reg: &str, offset: u32, size: u64) -> Result<Vec<String>, PinkerError> {
    if reg != "%rax" {
        return Err(err("registrador não suportado no storage de união"));
    }
    Ok(match size {
        1 => vec![format!("movzbq -{offset}(%rbp), {reg}")],
        2 => vec![format!("movzwq -{offset}(%rbp), {reg}")],
        4 => vec![format!("movl -{offset}(%rbp), %eax")],
        8 => vec![format!("movq -{offset}(%rbp), {reg}")],
        _ => {
            return Err(err(
                "subset externo montável só extrai payload escalar de união com 1, 2, 4 ou 8 bytes",
            ));
        }
    })
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
                | SelectedInstr::UnionInject { dest, .. }
                | SelectedInstr::UnionTag { dest, .. }
                | SelectedInstr::UnionExtract { dest, .. }
                | SelectedInstr::BitAnd { dest, .. }
                | SelectedInstr::BitOr { dest, .. }
                | SelectedInstr::BitXor { dest, .. }
                | SelectedInstr::Shl { dest, .. }
                | SelectedInstr::Shr { dest, .. }
                | SelectedInstr::Add { dest, .. }
                | SelectedInstr::PointerOffset { dest, .. }
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
                | SelectedInstr::Call { dest, .. }
                | SelectedInstr::CallIndirect { dest, .. }
                | SelectedInstr::MakeClosure { dest, .. }
                | SelectedInstr::MakeTraitObject { dest, .. } => {
                    ids.insert(temp_key(*dest));
                }
                SelectedInstr::TraitCall {
                    dest: Some(dest), ..
                } => {
                    ids.insert(temp_key(*dest));
                }
                SelectedInstr::CallRaw {
                    dest: Some(dest), ..
                } => {
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
    rodata_strings: &[ExternalCallConvString],
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
        OperandIR::Str(value) => {
            let Some(string_meta) = rodata_strings.iter().find(|entry| entry.value == *value)
            else {
                return Err(err(
                    "subset externo montável (Fase 135) não encontrou literal `verso` materializado em .rodata",
                ));
            };
            lines.push(format!("leaq {}(%rip), {}", string_meta.label, reg));
        }
        OperandIR::FunctionRef(name) => {
            lines.push(format!(
                "leaq {}(%rip), {}",
                function_ref_descriptor_label(name),
                reg
            ));
        }
        OperandIR::RawFunctionRef(name) => {
            let symbol = native_symbol::function_symbol(NativeSurface::Assemblable, name);
            lines.push(format!("leaq {}(%rip), {}", symbol, reg));
        }
    }
    Ok(lines)
}

// Fase 242: label determinístico do descritor estático {code_ptr, env_ptr}
// de uma função referenciada como valor callable. Não exige tabela de
// deduplicação (o nome da função já é único), ao contrário dos literais
// `verso` em .rodata.
fn function_ref_descriptor_label(name: &str) -> String {
    format!(".Lpinker_fnref_{}", name)
}

fn trait_symbol_component(name: &str) -> String {
    let mut component = String::with_capacity(name.len() * 2);
    for byte in name.as_bytes() {
        write!(&mut component, "{byte:02x}").expect("escrita em String não falha");
    }
    component
}

fn trait_vtable_symbol(trait_name: &str, concrete_type_name: &str) -> String {
    format!(
        ".Lpinker_trait_vtable_{}__{}",
        trait_symbol_component(trait_name),
        trait_symbol_component(concrete_type_name)
    )
}

fn register_trait_vtable(
    selected: &SelectedProgram,
    trait_name: &str,
    concrete_type_name: &str,
    concrete_type: TypeIR,
    methods: &[String],
    vtables: &mut BTreeMap<String, ExternalTraitVtable>,
    adapters: &mut BTreeMap<String, ExternalTraitAdapter>,
) -> Result<String, PinkerError> {
    if methods.is_empty() {
        return Err(err("backend nativo encontrou vtable de trato vazia"));
    }

    let symbol = trait_vtable_symbol(trait_name, concrete_type_name);
    let mut entries = Vec::with_capacity(methods.len());
    for (slot, target) in methods.iter().enumerate() {
        if !selected
            .functions
            .iter()
            .any(|function| &function.name == target)
        {
            return Err(err(
                "backend nativo encontrou método de vtable sem função sintética",
            ));
        }

        if trait_snapshot_uses_address(concrete_type) {
            entries.push(target.clone());
        } else {
            let adapter_symbol = format!("{}__slot_{}", symbol, slot);
            let adapter = ExternalTraitAdapter {
                symbol: adapter_symbol.clone(),
                target: target.clone(),
                concrete_type,
            };
            if let Some(previous) = adapters.insert(adapter_symbol.clone(), adapter.clone()) {
                if previous != adapter {
                    return Err(err(
                        "backend nativo encontrou colisão de adaptador de objeto de trato",
                    ));
                }
            }
            entries.push(adapter_symbol);
        }
    }

    let vtable = ExternalTraitVtable {
        symbol: symbol.clone(),
        entries,
    };
    if let Some(previous) = vtables.insert(symbol.clone(), vtable.clone()) {
        if previous != vtable {
            return Err(err(
                "backend nativo encontrou vtables divergentes para o mesmo trato e tipo concreto",
            ));
        }
    }
    Ok(symbol)
}

fn trait_snapshot_uses_address(concrete_type: TypeIR) -> bool {
    matches!(concrete_type, TypeIR::Struct | TypeIR::FixedArray { .. })
}

fn lower_trait_snapshot_copy(
    concrete_type: TypeIR,
    concrete_size: u64,
) -> Result<Vec<String>, PinkerError> {
    if concrete_size == 0 {
        return Err(err(
            "backend nativo recusou snapshot vazio de objeto de trato",
        ));
    }

    if !trait_snapshot_uses_address(concrete_type) {
        let instruction = match concrete_size {
            1 => "movb %r10b, 0(%rax)",
            2 => "movw %r10w, 0(%rax)",
            4 => "movl %r10d, 0(%rax)",
            8 => "movq %r10, 0(%rax)",
            _ => {
                return Err(err(
                    "backend nativo encontrou tamanho escalar inválido para snapshot",
                ));
            }
        };
        return Ok(vec![instruction.to_string()]);
    }

    let mut lines = Vec::new();
    let mut offset = 0_u64;
    while concrete_size - offset >= 8 {
        lines.push(format!("movq {}(%r10), %r11", offset));
        lines.push(format!("movq %r11, {}(%rax)", offset));
        offset += 8;
    }
    if concrete_size - offset >= 4 {
        lines.push(format!("movl {}(%r10), %r11d", offset));
        lines.push(format!("movl %r11d, {}(%rax)", offset));
        offset += 4;
    }
    if concrete_size - offset >= 2 {
        lines.push(format!("movw {}(%r10), %r11w", offset));
        lines.push(format!("movw %r11w, {}(%rax)", offset));
        offset += 2;
    }
    if concrete_size - offset == 1 {
        lines.push(format!("movb {}(%r10), %r11b", offset));
        lines.push(format!("movb %r11b, {}(%rax)", offset));
    }
    Ok(lines)
}

fn trait_adapter_receiver_load(concrete_type: TypeIR) -> &'static str {
    match concrete_type {
        TypeIR::U8 | TypeIR::Logica => "movzbq 0(%rdi), %rdi",
        TypeIR::I8 => "movsbq 0(%rdi), %rdi",
        TypeIR::U16 => "movzwq 0(%rdi), %rdi",
        TypeIR::I16 => "movswq 0(%rdi), %rdi",
        TypeIR::U32 => "movl 0(%rdi), %edi",
        TypeIR::I32 => "movslq 0(%rdi), %rdi",
        _ => "movq 0(%rdi), %rdi",
    }
}

fn normalize_sysv_scalar_argument(register: &str, ty: TypeIR) -> Result<Vec<String>, PinkerError> {
    let Some((reg8, reg16, reg32)) = (match register {
        "%rax" => Some(("%al", "%ax", "%eax")),
        "%rdi" => Some(("%dil", "%di", "%edi")),
        "%rsi" => Some(("%sil", "%si", "%esi")),
        "%rdx" => Some(("%dl", "%dx", "%edx")),
        "%rcx" => Some(("%cl", "%cx", "%ecx")),
        "%r8" => Some(("%r8b", "%r8w", "%r8d")),
        "%r9" => Some(("%r9b", "%r9w", "%r9d")),
        "%r10" => Some(("%r10b", "%r10w", "%r10d")),
        "%r11" => Some(("%r11b", "%r11w", "%r11d")),
        _ => None,
    }) else {
        return if matches!(
            ty,
            TypeIR::U8
                | TypeIR::I8
                | TypeIR::U16
                | TypeIR::I16
                | TypeIR::U32
                | TypeIR::I32
                | TypeIR::Logica
        ) {
            Err(err(
                "backend nativo não conhece subregistrador SysV para normalizar escalar",
            ))
        } else {
            Ok(Vec::new())
        };
    };

    let instruction = match ty {
        TypeIR::U8 | TypeIR::Logica => Some(format!("movzbq {}, {}", reg8, register)),
        TypeIR::I8 => Some(format!("movsbq {}, {}", reg8, register)),
        TypeIR::U16 => Some(format!("movzwq {}, {}", reg16, register)),
        TypeIR::I16 => Some(format!("movswq {}, {}", reg16, register)),
        TypeIR::U32 => Some(format!("movl {}, {}", reg32, reg32)),
        TypeIR::I32 => Some(format!("movslq {}, {}", reg32, register)),
        _ => None,
    };
    Ok(instruction.into_iter().collect())
}

fn normalize_inline_asm_output(register: &str, ty: TypeIR) -> Result<Vec<String>, PinkerError> {
    if ty != TypeIR::Logica {
        return normalize_sysv_scalar_argument(register, ty);
    }
    let byte_register = match register {
        "%rax" => "%al",
        "%rdi" => "%dil",
        "%rsi" => "%sil",
        "%rdx" => "%dl",
        "%rcx" => "%cl",
        "%r8" => "%r8b",
        "%r9" => "%r9b",
        "%r10" => "%r10b",
        "%r11" => "%r11b",
        _ => {
            return Err(err(
                "backend nativo não conhece subregistrador para saída lógica de sussurro",
            ));
        }
    };
    Ok(vec![
        format!("testq {register}, {register}"),
        format!("setne {byte_register}"),
        format!("movzbq {byte_register}, {register}"),
    ])
}

fn sysv_stack_layout(total_args: usize) -> (usize, usize) {
    let stack_args = total_args.saturating_sub(ARG_REGS.len());
    (stack_args, stack_args % 2)
}

fn temp_key(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}
// @pinker-nav:end backend-s.lowering.operandos-slots

// @pinker-nav:start backend-s.validacao.labels-tipos
// @pinker-nav:domain validacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary Validação de rótulos e predicados de tipo do caminho montável: `validate_external_block_labels` (recusa bloco sem label, label duplicado, exige bloco `entry`, valida alvos de `jmp`/`br` e a condição de `br`) e os predicados `is_supported_type`, `is_external_deref_load_type`/`_store_type`, `is_external_param_type`/`_local_type`/`_ret_type` e `is_external_call_ret_type` (retornos de função mais `nulo` para intrínsecas de efeito). Nomes de função/global são usados diretamente como símbolos, sem sanitização nesta camada; quem decide símbolo e ligação é `native_symbol`.
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
            | TypeIR::FunctionPointer
            | TypeIR::TraitObject
            | TypeIR::Union(_)
            | TypeIR::Nulo
    )
}

fn is_external_deref_load_type(ty: &TypeIR) -> bool {
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
            | TypeIR::Function
            | TypeIR::FunctionPointer
            | TypeIR::Pointer { .. }
            | TypeIR::TraitObject
            | TypeIR::Union(_)
    )
}

fn is_external_deref_store_type(ty: &TypeIR) -> bool {
    is_external_deref_load_type(ty)
}

fn is_external_legacy_deref_load_type(ty: &TypeIR) -> bool {
    matches!(
        ty,
        TypeIR::Bombom
            | TypeIR::U8
            | TypeIR::U32
            | TypeIR::U64
            | TypeIR::Function
            | TypeIR::TraitObject
    )
}

fn is_external_legacy_deref_store_type(ty: &TypeIR) -> bool {
    matches!(ty, TypeIR::Bombom | TypeIR::U8 | TypeIR::U32 | TypeIR::U64)
}

fn external_memory_width(ty: TypeIR) -> u64 {
    match ty {
        TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => 1,
        TypeIR::U16 | TypeIR::I16 => 2,
        TypeIR::U32 | TypeIR::I32 => 4,
        _ => 8,
    }
}

fn external_memory_alignment(ty: TypeIR) -> u64 {
    external_memory_width(ty)
}

fn is_external_param_type(ty: &TypeIR) -> bool {
    *ty == TypeIR::Bombom
        || *ty == TypeIR::U32
        || *ty == TypeIR::U64
        || *ty == TypeIR::Verso
        || *ty == TypeIR::Logica
        || *ty == TypeIR::ListBombom
        || *ty == TypeIR::ListVerso
        || *ty == TypeIR::MapVersoBombom
        || *ty == TypeIR::MapVersoVerso
        || *ty == TypeIR::MapBombomBombom
        || *ty == TypeIR::MapBombomVerso
        || matches!(ty, TypeIR::Map { .. })
        // Handles opacos são uma palavra na ABI, mas preservam sua identidade
        // nominal nas autoridades anteriores ao backend.
        || *ty == TypeIR::OpaqueWordHandle
        || *ty == TypeIR::Struct
        || *ty == TypeIR::Pointer { is_volatile: false }
        || *ty == TypeIR::Function
        || *ty == TypeIR::FunctionPointer
        || *ty == TypeIR::TraitObject
        || matches!(ty, TypeIR::Union(_))
}

fn is_external_scalar_param_type(ty: &TypeIR) -> bool {
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
    )
}

fn is_external_raw_call_type(ty: &TypeIR) -> bool {
    is_external_scalar_param_type(ty)
        || matches!(
            ty,
            TypeIR::Verso
                | TypeIR::ListBombom
                | TypeIR::ListVerso
                | TypeIR::MapVersoBombom
                | TypeIR::MapVersoVerso
                | TypeIR::MapBombomBombom
                | TypeIR::MapBombomVerso
                | TypeIR::Map { .. }
                | TypeIR::OpaqueWordHandle
                | TypeIR::Pointer { .. }
                | TypeIR::Function
                | TypeIR::FunctionPointer
                | TypeIR::TraitObject
                | TypeIR::Union(_)
        )
}

fn is_external_raw_call_ret_type(ty: &TypeIR) -> bool {
    *ty == TypeIR::Nulo || is_external_raw_call_type(ty)
}

fn is_external_trait_receiver_type(ty: &TypeIR) -> bool {
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
    ) || is_external_param_type(ty)
}

fn is_external_local_type(ty: &TypeIR) -> bool {
    is_external_param_type(ty)
        || is_external_scalar_param_type(ty)
        // HR3: um agregado de array fixo é representado por endereço, como
        // `ninho` opaco e `seta<T>`, e ocupa exatamente um slot de palavra. É o
        // que permite ao braço de `encaixe` receber o binding de um payload
        // estrutural no caminho montável.
        || matches!(ty, TypeIR::FixedArray { .. })
}

fn is_external_ret_type(ty: &TypeIR) -> bool {
    is_external_scalar_param_type(ty)
        || matches!(
            ty,
            TypeIR::Verso
                | TypeIR::Logica
                | TypeIR::ListBombom
                | TypeIR::ListVerso
                | TypeIR::MapVersoBombom
                | TypeIR::MapVersoVerso
                | TypeIR::MapBombomBombom
                | TypeIR::MapBombomVerso
                | TypeIR::Map { .. }
                | TypeIR::OpaqueWordHandle
                | TypeIR::Pointer { .. }
                | TypeIR::Function
                | TypeIR::FunctionPointer
                | TypeIR::TraitObject
                | TypeIR::Nulo
        )
}

/// Retornos aceitos em `call`: os de função mais `nulo` (intrínsecas de
/// efeito, como `lista_anexar`; o slot de destino recebe lixo inofensivo
/// que a semântica já impede de ser lido).
fn is_external_call_ret_type(ty: &TypeIR) -> bool {
    is_external_ret_type(ty) || matches!(ty, TypeIR::Nulo)
}
// @pinker-nav:end backend-s.validacao.labels-tipos

// @pinker-nav:start backend-s.runtime.intrinsecas-por-aridade
// @pinker-nav:domain runtime
// @pinker-nav:layer backend-s
// @pinker-nav:summary Autoridade de seleção de rota do subset externo montável. `runtime_intrinsic_symbol_por_aridade` resolve as intrínsecas cujo símbolo varia por número de argumentos: as superfícies de `falha_operacional` casam pela aridade exata, e o recorte nominal cobre `afirmar` (1|2, mensagem opcional), `executar_processo`, `capturar_stdout` e `capturar_stderr` (1|2) e `executar_com_entrada` (2|3). `is_arity_runtime_intrinsic` decide a elegibilidade pelo mesmo conjunto. `resolver_rota_de_chamada` compõe a precedência final — aridade, depois nome, depois função Pinker declarada — devolvendo `RotaDeChamada` e distinguindo aridade fora do recorte de callee desconhecido. `formatar_verso` não participa: D7 usa `pinker_formatar_verso_pack` para qualquer count representável.
/// Intrínsecas de aridade variável (Fases 219/B8 e 221/B10): o símbolo do
/// runtime é escolhido pela quantidade de argumentos no call site.
fn runtime_intrinsic_symbol_por_aridade(callee: &str, argc: usize) -> Option<String> {
    if let Some(superficie) = crate::falha_operacional::superficie(callee) {
        return (argc == superficie.aridade()).then(|| superficie.simbolo_runtime.to_string());
    }
    match (callee, argc) {
        ("afirmar", 1 | 2) => Some(format!("pinker_afirmar_{}", argc)),
        ("executar_processo", 1 | 2) => Some(format!("pinker_processo_executar_{}", argc)),
        ("capturar_stdout", 1 | 2) => Some(format!("pinker_processo_capturar_stdout_{}", argc)),
        ("capturar_stderr", 1 | 2) => Some(format!("pinker_processo_capturar_stderr_{}", argc)),
        ("executar_com_entrada", 2 | 3) => Some(format!("pinker_processo_com_entrada_{}", argc)),
        _ => None,
    }
}

fn is_arity_runtime_intrinsic(callee: &str) -> bool {
    crate::falha_operacional::superficie(callee).is_some()
        || matches!(
            callee,
            "afirmar"
                | "executar_processo"
                | "capturar_stdout"
                | "capturar_stderr"
                | "executar_com_entrada"
        )
}

/// Rota de destino de um call site do subset externo montável.
///
/// Autoridade única de seleção: intrínseca de runtime por aridade, intrínseca
/// por nome, função Pinker declarada ou callee desconhecido. `Call` e
/// `CallVoid` consomem esta mesma decisão.
#[derive(Debug, PartialEq, Eq)]
enum RotaDeChamada {
    /// Símbolo `pinker_*` do runtime nativo.
    Runtime(String),
    /// Função Pinker comum, chamada pelo próprio símbolo.
    FuncaoPinker(String),
    /// Intrínseca de runtime reconhecida, porém com aridade fora do recorte.
    AridadeForaDoRecorte,
    /// Callee que não é intrínseca nem função Pinker declarada.
    CalleeDesconhecido,
}

/// Resolve a rota de um call site preservando a precedência do subset:
/// aridade, depois nome, depois função Pinker declarada.
///
/// `funcao_pinker_declarada` é avaliada apenas quando as duas autoridades de
/// intrínseca não reconhecem o callee, preservando a avaliação preguiçosa dos
/// sítios produtivos.
fn resolver_rota_de_chamada(
    callee: &str,
    argc: usize,
    funcao_pinker_declarada: impl FnOnce() -> bool,
) -> RotaDeChamada {
    if is_arity_runtime_intrinsic(callee) {
        return match runtime_intrinsic_symbol_por_aridade(callee, argc) {
            Some(simbolo) => RotaDeChamada::Runtime(simbolo),
            None => RotaDeChamada::AridadeForaDoRecorte,
        };
    }
    if let Some(simbolo) = runtime_intrinsic_symbol(callee) {
        return RotaDeChamada::Runtime(simbolo.to_string());
    }
    if funcao_pinker_declarada() {
        RotaDeChamada::FuncaoPinker(callee.to_string())
    } else {
        RotaDeChamada::CalleeDesconhecido
    }
}

// @pinker-nav:end backend-s.runtime.intrinsecas-por-aridade

// @pinker-nav:start backend-s.runtime.simbolos-intrinsecas
// @pinker-nav:domain runtime
// @pinker-nav:layer backend-s
// @pinker-nav:summary `runtime_intrinsic_symbol`: catálogo estático extenso que mapeia nomes de intrínsecas Pinker para símbolos `pinker_*` do runtime nativo (texto/`verso`, listas, mapas por chave `verso`/`bombom`, iteradores internos, arquivo/caminho, tempo, acaso, ambiente e leques). Uma única palavra de 8 bytes por elemento faz `lista<bombom>` e `lista<verso>` compartilharem os mesmos símbolos. Funções Pinker comuns não são intrínsecas (retornam `None` → símbolo direto). Mapear um símbolo **não** prova paridade completa da implementação nativa; o runtime não foi cartografado nesta onda. Região única — sem uma âncora por intrínseca.
/// Intrínsecas com implementação no runtime nativo (Fases 215/B4 e 216/B5).
/// O símbolo devolvido é resolvido no link com `libpinker_rt.a`.
///
/// As listas compartilham uma única implementação: todo elemento é uma palavra
/// de 8 bytes (`bombom`, ponteiro de `verso` ou valor de leque), então as
/// formas monomorphizadas de `lista<bombom>` e `lista<verso>` — e as genéricas
/// já reescritas na IR — abaixam para as mesmas funções `pinker_lista_*`.
fn runtime_intrinsic_symbol(callee: &str) -> Option<&'static str> {
    // Parte E1: a família JSON declara nome, assinatura e símbolo numa
    // autoridade só. O backend consulta em vez de manter uma cópia da lista.
    if let Some(simbolo) = crate::valor_json::simbolo_runtime(callee) {
        return Some(simbolo);
    }
    // Parte E2: mesma disciplina — a família SHA-256 declara nome e símbolo
    // numa autoridade só e o backend consulta em vez de copiar a lista.
    if let Some(simbolo) = crate::sha256::simbolo_runtime(callee) {
        return Some(simbolo);
    }
    match callee {
        "alocar" => Some("pinker_publico_alocar"),
        "liberar" => Some("pinker_publico_liberar"),
        "juntar_verso" => Some("pinker_verso_juntar"),
        "tamanho_verso" => Some("pinker_verso_tamanho"),
        "igual_verso" => Some("pinker_verso_igual"),
        // Família texto completa (Fase 219/B8).
        "indice_verso" => Some("pinker_verso_indice"),
        "fatiar_verso" => Some("pinker_verso_fatiar"),
        "contem_verso" => Some("pinker_verso_contem"),
        "comeca_com" => Some("pinker_verso_comeca_com"),
        "termina_com" => Some("pinker_verso_termina_com"),
        "vazio_verso" => Some("pinker_verso_vazio"),
        "nao_vazio_verso" => Some("pinker_verso_nao_vazio"),
        "aparar_verso" => Some("pinker_verso_aparar"),
        "minusculo_verso" => Some("pinker_verso_minusculo"),
        "maiusculo_verso" => Some("pinker_verso_maiusculo"),
        "indice_verso_em" => Some("pinker_verso_indice_em"),
        "buscar_verso" => Some("pinker_verso_buscar"),
        "dividir_verso_em" => Some("pinker_verso_dividir_em"),
        "dividir_verso_contar" => Some("pinker_verso_dividir_contar"),
        "substituir_verso" => Some("pinker_verso_substituir"),
        "juntar_verso_com" => Some("pinker_verso_juntar_com"),
        "verso_para_bombom" => Some("pinker_verso_para_bombom"),
        "bombom_para_verso" => Some("pinker_bombom_para_verso"),
        "lista_bombom_criar" | "lista_verso_criar" => Some("pinker_lista_criar"),
        "lista_bombom_anexar" | "lista_verso_anexar" => Some("pinker_lista_anexar"),
        "lista_bombom_obter" | "lista_verso_obter" => Some("pinker_lista_obter"),
        "lista_bombom_tamanho" | "lista_verso_tamanho" => Some("pinker_lista_tamanho"),
        "lista_bombom_definir" | "lista_verso_definir" => Some("pinker_lista_definir"),
        "lista_bombom_tirar_ultimo" | "lista_verso_tirar_ultimo" => {
            Some("pinker_lista_tirar_ultimo")
        }
        "lista_bombom_inserir" | "lista_verso_inserir" => Some("pinker_lista_inserir"),
        "emitir_linha_csv_bombom" => Some("pinker_emitir_linha_csv_bombom"),
        "ler_linha_csv_bombom" => Some("pinker_ler_linha_csv_bombom"),
        // Mapas (Fase 217/B6): chave `verso` compara por conteúdo, chave
        // `bombom` por valor; os 4 tipos compartilham as demais operações.
        "mapa_verso_bombom_criar" | "mapa_verso_verso_criar" => {
            Some("pinker_mapa_criar_chave_verso")
        }
        "mapa_bombom_bombom_criar" | "mapa_bombom_verso_criar" => {
            Some("pinker_mapa_criar_chave_bombom")
        }
        "__pinker_internal_mapa_criar_chave_verso" => Some("pinker_mapa_criar_chave_verso"),
        "__pinker_internal_mapa_criar_chave_bombom" => Some("pinker_mapa_criar_chave_bombom"),
        "mapa_verso_bombom_definir"
        | "mapa_verso_verso_definir"
        | "mapa_bombom_bombom_definir"
        | "mapa_bombom_verso_definir" => Some("pinker_mapa_definir"),
        "__pinker_internal_mapa_definir" => Some("pinker_mapa_definir"),
        "mapa_verso_bombom_obter"
        | "mapa_verso_verso_obter"
        | "mapa_bombom_bombom_obter"
        | "mapa_bombom_verso_obter" => Some("pinker_mapa_obter"),
        "__pinker_internal_mapa_obter" => Some("pinker_mapa_obter"),
        "mapa_verso_bombom_tem"
        | "mapa_verso_verso_tem"
        | "mapa_bombom_bombom_tem"
        | "mapa_bombom_verso_tem" => Some("pinker_mapa_tem"),
        "__pinker_internal_mapa_tem" => Some("pinker_mapa_tem"),
        "mapa_verso_bombom_tamanho"
        | "mapa_verso_verso_tamanho"
        | "mapa_bombom_bombom_tamanho"
        | "mapa_bombom_verso_tamanho" => Some("pinker_mapa_tamanho"),
        "__pinker_internal_mapa_tamanho" => Some("pinker_mapa_tamanho"),
        "mapa_verso_bombom_remover"
        | "mapa_verso_verso_remover"
        | "mapa_bombom_bombom_remover"
        | "mapa_bombom_verso_remover" => Some("pinker_mapa_remover"),
        "__pinker_internal_mapa_remover" => Some("pinker_mapa_remover"),
        "__pinker_internal_mapa_verso_bombom_iterador_criar"
        | "__pinker_internal_mapa_verso_verso_iterador_criar"
        | "__pinker_internal_mapa_bombom_bombom_iterador_criar"
        | "__pinker_internal_mapa_bombom_verso_iterador_criar" => {
            Some("pinker_mapa_iterador_criar")
        }
        "__pinker_internal_mapa_iterador_criar" => Some("pinker_mapa_iterador_criar"),
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave"
        | "__pinker_internal_mapa_verso_verso_iterador_proxima_chave"
        | "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave"
        | "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave" => {
            Some("pinker_mapa_iterador_proxima")
        }
        "__pinker_internal_mapa_iterador_proxima_chave_bombom"
        | "__pinker_internal_mapa_iterador_proxima_chave_verso" => {
            Some("pinker_mapa_iterador_proxima")
        }
        // Arquivo, caminho, tempo e acaso (Fase 220/B9).
        "abrir" => Some("pinker_arquivo_abrir"),
        "criar_arquivo" => Some("pinker_arquivo_criar"),
        "abrir_anexo" => Some("pinker_arquivo_abrir_anexo"),
        "fechar" => Some("pinker_arquivo_fechar"),
        "ler_arquivo" => Some("pinker_arquivo_ler_bombom"),
        "ler_verso_arquivo" => Some("pinker_arquivo_ler_verso"),
        "escrever" => Some("pinker_arquivo_escrever_bombom"),
        "escrever_verso" => Some("pinker_arquivo_escrever_verso"),
        "truncar_arquivo" => Some("pinker_arquivo_truncar"),
        "anexar_verso" => Some("pinker_arquivo_anexar_verso"),
        "ler_arquivo_verso" => Some("pinker_arquivo_ler_caminho_verso"),
        "processo_codigo" => Some("pinker_saida_processo_codigo"),
        "processo_saida" => Some("pinker_saida_processo_stdout"),
        "processo_erro" => Some("pinker_saida_processo_stderr"),
        "arquivo_ou" => Some("pinker_arquivo_ou"),
        "copiar_arquivo" => Some("pinker_arquivo_copiar"),
        "renomear_arquivo" => Some("pinker_arquivo_renomear"),
        "caminho_existe" => Some("pinker_caminho_existe"),
        "e_arquivo" => Some("pinker_caminho_e_arquivo"),
        "e_diretorio" => Some("pinker_caminho_e_diretorio"),
        "juntar_caminho" => Some("pinker_caminho_juntar"),
        "tamanho_arquivo" => Some("pinker_caminho_tamanho_arquivo"),
        "e_vazio" => Some("pinker_caminho_e_vazio"),
        "criar_diretorio" => Some("pinker_caminho_criar_diretorio"),
        "remover_arquivo" => Some("pinker_caminho_remover_arquivo"),
        "remover_diretorio" => Some("pinker_caminho_remover_diretorio"),
        "diretorio_atual" => Some("pinker_caminho_diretorio_atual"),
        "tempo_unix" => Some("pinker_tempo_unix"),
        "formatar_tempo_unix" => Some("pinker_formatar_tempo_unix"),
        "dormir" => Some("pinker_dormir"),
        "aleatorio_criar" => Some("pinker_aleatorio_criar"),
        "aleatorio_proximo" => Some("pinker_aleatorio_proximo"),
        "aleatorio_entre" => Some("pinker_aleatorio_entre"),
        // Ambiente (Fase 221/B10) — argv/env capturados por pinker_rt_iniciar.
        "quantos_argumentos" => Some("pinker_ambiente_quantos_argumentos"),
        "argumento" => Some("pinker_ambiente_argumento"),
        "argumento_ou" => Some("pinker_ambiente_argumento_ou"),
        "tem_argumento" => Some("pinker_ambiente_tem_argumento"),
        "tem_chave" | "tem_argumento_nomeado" => Some("pinker_ambiente_tem_chave"),
        "pedir_argumento" | "argumento_nomeado_ou" => Some("pinker_ambiente_pedir_argumento"),
        "tem_flag" => Some("pinker_ambiente_tem_flag"),
        "ambiente_ou" => Some("pinker_ambiente_ou"),
        "buscar_contexto" | "argumento_nomeado_ou_ambiente_ou" => {
            Some("pinker_ambiente_buscar_contexto")
        }
        "pipeline_minimo" => Some("pinker_processo_pipeline"),
        "sair" => Some("pinker_sair"),
        // Leques com carga (Fase 218/B7): anexar e carga não distinguem
        // bombom/verso no runtime — toda carga é uma palavra de 8 bytes.
        "__pinker_internal_leque_criar_0" => Some("pinker_leque_criar_0"),
        // D1: handles de lista entram pelos **mesmos** símbolos. O caminho de
        // carga de uma palavra já transporta qualquer handle opaco sem tocar na
        // ABI: `pinker_leque_anexar`/`pinker_leque_carga` movem um `u64` e não
        // interpretam o conteúdo. Nenhum símbolo novo é criado.
        "__pinker_internal_leque_anexar_b"
        | "__pinker_internal_leque_anexar_v"
        | "__pinker_internal_leque_anexar_lista_b"
        | "__pinker_internal_leque_anexar_lista_v"
        | "__pinker_internal_leque_anexar_saida_processo" => Some("pinker_leque_anexar"),
        "__pinker_internal_leque_tag" => Some("pinker_leque_tag"),
        "__pinker_internal_leque_carga_b"
        | "__pinker_internal_leque_carga_v"
        | "__pinker_internal_leque_carga_lista_b"
        | "__pinker_internal_leque_carga_lista_v"
        | "__pinker_internal_leque_carga_saida_processo" => Some("pinker_leque_carga"),
        // As uniões não passam por este mapeamento: `union_tag` e
        // `union_extract` são operações internas tipadas, e o símbolo
        // (`pinker_uniao_tag`/`pinker_uniao_payload_b`/`..._v`) é escolhido
        // diretamente no lowering do backend.
        _ => None,
    }
}
// @pinker-nav:end backend-s.runtime.simbolos-intrinsecas

// @pinker-nav:start backend-s.dados.strings-rodata
// @pinker-nav:domain dados
// @pinker-nav:layer backend-s
// @pinker-nav:summary Deduplicação e escape de literais `verso` para `.rodata`: `collect_rodata_string_label` (deduplica por valor, cria labels `.Lpinker_verso_N` e registra o operando textual), `register_rodata_strings_for_operand` (registra quando o operando é `Str`) e `escape_gas_string` (escapa `\`, `"`, `\n`, `\t` para o GAS; caracteres de controle não tratados explicitamente passam crus). Sustenta o layout `[u64 tamanho][bytes]` do renderer montável.
fn collect_rodata_string_label(
    value: &str,
    labels: &mut HashMap<String, String>,
    strings: &mut Vec<ExternalCallConvString>,
) -> String {
    if let Some(existing) = labels.get(value) {
        return existing.clone();
    }
    let label = format!(".Lpinker_verso_{}", strings.len());
    labels.insert(value.to_string(), label.clone());
    strings.push(ExternalCallConvString {
        label: label.clone(),
        value: value.to_string(),
    });
    label
}

fn register_rodata_strings_for_operand(
    operand: &OperandIR,
    labels: &mut HashMap<String, String>,
    strings: &mut Vec<ExternalCallConvString>,
) {
    if let OperandIR::Str(value) = operand {
        let _ = collect_rodata_string_label(value, labels, strings);
    }
}

fn escape_gas_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
// @pinker-nav:end backend-s.dados.strings-rodata

// @pinker-nav:start backend-s.renderizacao.abi-textual-programa
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `render_program`: renderer do `.s` **textual** baseado em `BackendTextProgram` (caminho `emit_from_selected`), distinto do renderer montável. Emite cabeçalho, `module`, `mode` livre/hospedado, metadados `abi.*` **como comentários** (`; abi.func`/`abi.params`/`abi.ret`/`abi.frame`/`abi.prologue`/`abi.epilogue`), `.rodata` de globais e blocos. No modo freestanding embute `boot.entry`, o linker script e o kernel stub textuais e um loop `.Lpinker_hang`. **Não** é assembly GAS montável nem ABI SysV real: `mov $slot`/`unop`/`binop` e os `@arg`/`@ret` são convenções textuais, não reconhecíveis diretamente pelo assembler.
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
            &native_symbol::native_binding(NativeDefinition::Entrypoint)
                .directive(FREESTANDING_BOOT_ENTRY_SYMBOL),
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
            line(
                &mut out,
                0,
                &native_symbol::native_binding(NativeDefinition::UserGlobal)
                    .directive(&global.name),
            );
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
        let symbol = native_symbol::function_symbol(NativeSurface::TextualAbi, &function.name);
        let prologue = native_symbol::injective_local_label(&[&function.name, "prologue"]);
        let epilogue = native_symbol::injective_local_label(&[&function.name, "epilogue"]);
        line(
            &mut out,
            0,
            &format!("; abi.frame prologue={} epilogue={}", prologue, epilogue),
        );
        line(
            &mut out,
            0,
            &native_symbol::function_binding(&function.name).directive(&symbol),
        );
        line(&mut out, 0, &format!("{}:", symbol));
        line(&mut out, 1, &format!("{}:", prologue));
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
                &format!(
                    "{}:",
                    native_symbol::injective_local_label(&[&function.name, &block.label])
                ),
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
        line(&mut out, 1, &format!("{}:", epilogue));
        line(&mut out, 2, "; abi.epilogue (textual)");
    }

    out
}
// @pinker-nav:end backend-s.renderizacao.abi-textual-programa

// @pinker-nav:start backend-s.renderizacao.abi-textual-instrucoes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `render_instruction` e `render_terminator` do `.s` textual: formatam cada `BackendTextInstruction` (`mov`, `unop`, `binop`, `call ; abi.call ... -> ...` com ramo defensivo de call inválida, `falar` com pares `valor:tipo`) e cada `BackendTextTerminator` (`jmp`, `br`, `ret @ret`, `ret_void`). Convenções textuais anotadas — não emitem instruções x86 reais.
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
        crate::backend_text::BackendTextInstruction::PointerOffset {
            dest,
            pointer,
            offset,
            element_size,
            element_align,
        } => format!(
            "pointer_offset {}, {}, {}, size={}, align={}",
            render_temp(*dest),
            render_operand(pointer),
            render_operand(offset),
            element_size,
            element_align
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
        crate::backend_text::BackendTextInstruction::CallRaw {
            dest,
            callee,
            args,
            param_types,
            ret_type,
        } => {
            let call = format!(
                "call_raw {}({}) : ({}) -> {}",
                render_operand(callee),
                args.iter()
                    .map(render_operand)
                    .collect::<Vec<_>>()
                    .join(", "),
                param_types
                    .iter()
                    .map(TypeIR::render_name)
                    .collect::<Vec<_>>()
                    .join(", "),
                ret_type.render_name()
            );
            match dest {
                Some(dest) => format!("{} -> {}", call, render_temp(*dest)),
                None => call,
            }
        }
        crate::backend_text::BackendTextInstruction::MakeTraitObject {
            dest,
            value,
            trait_name,
            concrete_type_name,
            concrete_size,
            vtable_methods,
            ..
        } => format!(
            "make_trait_object {} <- {} as trato<{}> snapshot={}({}) vtable=[{}]",
            render_temp(*dest),
            render_operand(value),
            trait_name,
            concrete_size,
            concrete_type_name,
            vtable_methods.join(", ")
        ),
        crate::backend_text::BackendTextInstruction::TraitCall {
            dest,
            object,
            trait_name,
            method_name,
            method_slot,
            args,
            ret_type,
            ..
        } => {
            let call_site = format!(
                "trait_call trato<{}>.{}[{}]({})",
                trait_name,
                method_name,
                method_slot,
                args.iter()
                    .map(render_operand)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            match (dest, ret_type) {
                (Some(dest), _) => format!(
                    "{} {} ; abi.trait_call object={} -> {}",
                    call_site,
                    ret_type.name(),
                    render_operand(object),
                    render_temp(*dest)
                ),
                (None, TypeIR::Nulo) => format!(
                    "{} void ; abi.trait_call object={}",
                    call_site,
                    render_operand(object)
                ),
                (None, _) => format!("; trait_call inválida: {}", call_site),
            }
        }
        crate::backend_text::BackendTextInstruction::Falar { args } => format!(
            "falar {}",
            args.iter()
                .map(|arg| format!("{}:{}", render_operand(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        crate::backend_text::BackendTextInstruction::InlineAsm {
            chunks,
            operands,
            clobbers,
        } => {
            format!(
                "inline_asm {:?} operands={} clobbers={:?}",
                chunks,
                operands.len(),
                clobbers
            )
        }
        crate::backend_text::BackendTextInstruction::UnionInject {
            dest,
            value,
            union_type_id,
            tag,
        } => format!(
            "union_inject %{} #{} tag={} {}",
            dest.0,
            union_type_id.0,
            tag,
            render_operand(value)
        ),
        crate::backend_text::BackendTextInstruction::UnionTag {
            dest,
            value,
            union_type_id,
        } => format!(
            "union_tag %{} #{} {}",
            dest.0,
            union_type_id.0,
            render_operand(value)
        ),
        crate::backend_text::BackendTextInstruction::UnionExtract {
            dest,
            value,
            union_type_id,
            tag,
            canonical_member_key,
            payload_type,
        } => format!(
            "union_extract %{} #{} tag={} key={} {} -> {}",
            dest.0,
            union_type_id.0,
            tag,
            canonical_member_key,
            render_operand(value),
            payload_type.name()
        ),
    }
}

fn render_terminator(
    term: &crate::backend_text::BackendTextTerminator,
    function_name: &str,
) -> String {
    match term {
        crate::backend_text::BackendTextTerminator::Jump(label) => {
            format!(
                "jmp {}",
                native_symbol::injective_local_label(&[function_name, label])
            )
        }
        crate::backend_text::BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            native_symbol::injective_local_label(&[function_name, then_label]),
            native_symbol::injective_local_label(&[function_name, else_label])
        ),
        crate::backend_text::BackendTextTerminator::Return(Some(value)) => {
            format!("ret @ret, {}", render_operand(value))
        }
        crate::backend_text::BackendTextTerminator::Return(None) => "ret_void".to_string(),
    }
}
// @pinker-nav:end backend-s.renderizacao.abi-textual-instrucoes

// @pinker-nav:start backend-s.renderizacao.abi-textual-componentes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary Componentes do renderer `.s` textual: `render_unary`/`render_binop` (nomes de operador), `render_operand` (locais `$slot`, globais `@nome(%rip)`, inteiros, `1`/`0`, strings entre aspas **sem escape**, temporários `%tN`), `render_temp`, `render_slot`, `join_or_empty` e os helpers de metadado `render_abi_params`/`render_abi_return`/`render_call_site`/`render_abi_call_args` (`@arg`/`@ret`, comentários). Serializam elementos individuais da representação textual; não produzem código nativo.
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
        crate::cfg_ir::OperandIR::FunctionRef(name) => format!("fnref({})", name),
        crate::cfg_ir::OperandIR::RawFunctionRef(name) => format!("raw_fnref({})", name),
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
// @pinker-nav:end backend-s.renderizacao.abi-textual-componentes

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

// @pinker-nav:start evidencia.backend-s.proveniencia-de-ponteiro
// @pinker-nav:domain memoria
// @pinker-nav:layer evidencia
// @pinker-nav:summary Unidade da classificação de proveniência do back-end nativo (continuação do hotfix pós-PR #411): `selected_call_provenance` como autoridade única sobre chamada direta, indireta, por endereço cru e de trato — `Public` quando e somente quando o retorno é ponteiro —, e a regra do cast `virar seta<T>`, que preserva `Public`, `Internal`, `Fabricated` e `Unclassified` tipado como ponteiro, e só produz `Fabricated` a partir de valor não-ponteiro. Cobre os ramos que a superfície da linguagem ainda não alcança, porque `seta<seta<T>>`, carga de ponteiro pela memória e carga de união com ponteiro estão fora do subconjunto atual.
#[cfg(test)]
mod tests_proveniencia_de_ponteiro {
    use super::*;
    use crate::cfg_ir::{OperandIR, TempIR};
    use crate::instr_select::{SelectedBlock, SelectedFunction, SelectedTerminator};

    const PONTEIRO: TypeIR = TypeIR::Pointer { is_volatile: false };
    const OUTRO_PONTEIRO: TypeIR = TypeIR::Pointer { is_volatile: true };

    fn funcao(
        instructions: Vec<SelectedInstr>,
        slot_types: &[(&str, TypeIR)],
        internos: &[&str],
    ) -> SelectedFunction {
        SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: Vec::new(),
            locals: slot_types
                .iter()
                .map(|(nome, _)| nome.to_string())
                .collect(),
            slot_types: slot_types
                .iter()
                .map(|(nome, ty)| (nome.to_string(), *ty))
                .collect(),
            internal_pointer_params: internos.iter().map(|nome| nome.to_string()).collect(),
            blocks: vec![SelectedBlock {
                label: "entrada".to_string(),
                instructions,
                terminator: SelectedTerminator::Ret(None),
            }],
        }
    }

    fn proveniencia(function: &SelectedFunction, temp: TempIR) -> PointerProvenance {
        let mut visiting_temps = HashSet::new();
        let mut visiting_slots = HashSet::new();
        selected_temp_provenance(function, temp, &mut visiting_temps, &mut visiting_slots)
    }

    fn cast(dest: u32, value: OperandIR, target_type: TypeIR) -> SelectedInstr {
        SelectedInstr::Cast {
            dest: TempIR(dest),
            value,
            target_type,
        }
    }

    /// As quatro formas de chamada que devolvem valor, com o mesmo tipo de
    /// retorno, precisam produzir a mesma proveniência. A assimetria anterior
    /// classificava só a direta como `Public`, e o acesso pelas outras descia
    /// sem validação.
    fn chamadas_que_devolvem(ret_type: TypeIR) -> Vec<(&'static str, SelectedInstr)> {
        vec![
            (
                "direta",
                SelectedInstr::Call {
                    dest: TempIR(0),
                    callee: "fabricar".to_string(),
                    args: Vec::new(),
                    ret_type,
                },
            ),
            (
                "indireta",
                SelectedInstr::CallIndirect {
                    dest: TempIR(0),
                    callee: OperandIR::Local("f".to_string()),
                    args: Vec::new(),
                    ret_type,
                },
            ),
            (
                "crua",
                SelectedInstr::CallRaw {
                    dest: Some(TempIR(0)),
                    callee: OperandIR::Local("fp".to_string()),
                    args: Vec::new(),
                    param_types: Vec::new(),
                    ret_type,
                },
            ),
            (
                "trato",
                SelectedInstr::TraitCall {
                    dest: Some(TempIR(0)),
                    object: OperandIR::Local("objeto".to_string()),
                    trait_name: "Fonte".to_string(),
                    method_name: "regiao".to_string(),
                    method_slot: 0,
                    method_count: 1,
                    args: Vec::new(),
                    param_types: Vec::new(),
                    ret_type,
                },
            ),
        ]
    }

    #[test]
    fn toda_forma_de_chamada_que_devolve_ponteiro_e_publica() {
        for (forma, instrucao) in chamadas_que_devolvem(PONTEIRO) {
            let function = funcao(vec![instrucao], &[], &[]);
            assert_eq!(
                proveniencia(&function, TempIR(0)),
                PointerProvenance::Public,
                "chamada {forma} devolvendo ponteiro precisa ser pública"
            );
            assert!(
                proveniencia(&function, TempIR(0)).requires_access_check(),
                "chamada {forma}: o acesso precisa ser validado"
            );
        }
    }

    #[test]
    fn chamada_que_nao_devolve_ponteiro_nunca_e_publica() {
        for (forma, instrucao) in chamadas_que_devolvem(TypeIR::U64) {
            let function = funcao(vec![instrucao], &[], &[]);
            assert_eq!(
                proveniencia(&function, TempIR(0)),
                PointerProvenance::Unclassified,
                "chamada {forma} devolvendo inteiro não pode virar pública"
            );
        }
    }

    /// `alocar` continua público mesmo se o tipo de retorno não for anotado
    /// como ponteiro: é a origem canônica de região pública.
    #[test]
    fn alocar_permanece_publico_pelo_nome() {
        let function = funcao(
            vec![SelectedInstr::Call {
                dest: TempIR(0),
                callee: "alocar".to_string(),
                args: Vec::new(),
                ret_type: TypeIR::U64,
            }],
            &[],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Public
        );
    }

    #[test]
    fn cast_de_inteiro_para_ponteiro_fabrica_endereco() {
        let function = funcao(vec![cast(0, OperandIR::Int(4096), PONTEIRO)], &[], &[]);
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Fabricated
        );
        assert!(proveniencia(&function, TempIR(0)).requires_access_check());
    }

    #[test]
    fn cast_de_slot_inteiro_para_ponteiro_fabrica_endereco() {
        let function = funcao(
            vec![cast(0, OperandIR::Local("n".to_string()), PONTEIRO)],
            &[("n", TypeIR::U64)],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(0)),
            PointerProvenance::Fabricated
        );
    }

    /// Contrato central do Ponto 1: em cast ponteiro→ponteiro a proveniência da
    /// origem é preservada, incluindo `Unclassified`. Antes, `Unclassified`
    /// virava `Fabricated` só por falta de informação.
    #[test]
    fn cast_ponteiro_para_ponteiro_preserva_a_proveniencia() {
        // `Public`: chamada que devolve ponteiro.
        let publica = funcao(
            vec![
                SelectedInstr::Call {
                    dest: TempIR(0),
                    callee: "fabricar".to_string(),
                    args: Vec::new(),
                    ret_type: PONTEIRO,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[],
            &[],
        );
        assert_eq!(proveniencia(&publica, TempIR(1)), PointerProvenance::Public);

        // `Internal`: parâmetro de ambiente de closure.
        let interna = funcao(
            vec![cast(
                0,
                OperandIR::Local("__env".to_string()),
                OUTRO_PONTEIRO,
            )],
            &[("__env", PONTEIRO)],
            &["__env"],
        );
        assert_eq!(
            proveniencia(&interna, TempIR(0)),
            PointerProvenance::Internal
        );
        assert!(
            !proveniencia(&interna, TempIR(0)).requires_access_check(),
            "o domínio interno não pode ser confrontado com o registro público"
        );

        // `Fabricated`: cadeia inteiro → ponteiro A → ponteiro B.
        let fabricada = funcao(
            vec![
                cast(0, OperandIR::Int(4096), PONTEIRO),
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[],
            &[],
        );
        assert_eq!(
            proveniencia(&fabricada, TempIR(1)),
            PointerProvenance::Fabricated
        );

        // `Unclassified` **tipado como ponteiro**: origem carregada de memória.
        // Este ramo não é alcançável pela superfície atual da linguagem — é
        // exatamente por isso que a evidência é de unidade.
        let nao_classificada = funcao(
            vec![
                SelectedInstr::DerefLoad {
                    dest: TempIR(0),
                    ptr: OperandIR::Local("celula".to_string()),
                    ty: PONTEIRO,
                    is_volatile: false,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
            ],
            &[("celula", PONTEIRO)],
            &[],
        );
        assert_eq!(
            proveniencia(&nao_classificada, TempIR(0)),
            PointerProvenance::Unclassified,
            "carga de memória não é classificada pela análise atual"
        );
        assert_eq!(
            proveniencia(&nao_classificada, TempIR(1)),
            PointerProvenance::Unclassified,
            "cast ponteiro→ponteiro não pode promover a classe por falta de informação"
        );
        assert!(
            !proveniencia(&nao_classificada, TempIR(1)).requires_access_check(),
            "a classe não classificada permanece fora da validação pública"
        );
    }

    /// Cadeia longa: ponteiro não classificado atravessa dois casts sem trocar
    /// de classe.
    #[test]
    fn cadeia_de_casts_nao_promove_ponteiro_nao_classificado() {
        let function = funcao(
            vec![
                SelectedInstr::DerefLoad {
                    dest: TempIR(0),
                    ptr: OperandIR::Local("celula".to_string()),
                    ty: PONTEIRO,
                    is_volatile: false,
                },
                cast(1, OperandIR::Temp(TempIR(0)), OUTRO_PONTEIRO),
                cast(2, OperandIR::Temp(TempIR(1)), PONTEIRO),
            ],
            &[("celula", PONTEIRO)],
            &[],
        );
        assert_eq!(
            proveniencia(&function, TempIR(2)),
            PointerProvenance::Unclassified
        );
    }

    /// O resultado de uma comparação é lógico, não ponteiro: convertê-lo em
    /// `seta<T>` fabrica endereço. Confundir o tipo dos operandos com o tipo do
    /// destino faria uma comparação de ponteiros escapar da validação.
    #[test]
    fn comparacao_de_ponteiros_nao_e_ponteiro() {
        let function = funcao(
            vec![
                SelectedInstr::CmpEq {
                    dest: TempIR(0),
                    lhs: OperandIR::Local("a".to_string()),
                    rhs: OperandIR::Local("b".to_string()),
                    ty: PONTEIRO,
                },
                cast(1, OperandIR::Temp(TempIR(0)), PONTEIRO),
            ],
            &[("a", PONTEIRO), ("b", PONTEIRO)],
            &[],
        );
        assert_eq!(
            selected_temp_type(&function, TempIR(0)),
            Some(TypeIR::Logica)
        );
        assert_eq!(
            proveniencia(&function, TempIR(1)),
            PointerProvenance::Fabricated
        );
    }

    /// A autoridade única precisa reconhecer todas as formas de chamada e
    /// recusar o que não é chamada.
    #[test]
    fn autoridade_de_chamada_cobre_as_formas_e_ignora_o_resto() {
        for (forma, instrucao) in chamadas_que_devolvem(PONTEIRO) {
            let shape = selected_call_shape(&instrucao)
                .unwrap_or_else(|| panic!("forma {forma} precisa ser reconhecida"));
            assert_eq!(shape.dest, Some(TempIR(0)));
            assert_eq!(shape.ret_type, PONTEIRO);
        }
        assert!(selected_call_shape(&SelectedInstr::CallVoid {
            callee: "falar".to_string(),
            args: Vec::new(),
        })
        .is_none());
        assert!(selected_call_shape(&cast(0, OperandIR::Int(1), PONTEIRO)).is_none());
    }
}
// @pinker-nav:end evidencia.backend-s.proveniencia-de-ponteiro

// @pinker-nav:start evidencia.backend-s.selecao-de-rota-nativa
// @pinker-nav:domain lowering
// @pinker-nav:layer evidencia
// @pinker-nav:summary Unidade da autoridade de seleção de rota do subset externo montável (Issue #522): `resolver_rota_de_chamada` decide entre intrínseca de runtime por aridade, intrínseca por nome, função Pinker declarada e callee desconhecido, e é a mesma decisão consumida por `Call` e `CallVoid`. Cobre as cinco rotas reparadas, a precedência entre autoridades, a recusa de aridade fora do recorte, a não captura de função Pinker ordinária, a rejeição de callee desconhecido e a ausência estrutural das três exclusões `ouvir*` em todas as autoridades de despacho nativo, com probes de sensibilidade reversíveis que ficam vermelhos se o conjunto reconhecido for ampliado.
#[cfg(test)]
mod tests_selecao_de_rota_nativa {
    use super::*;

    fn rota(callee: &str, argc: usize, declarada: bool) -> RotaDeChamada {
        resolver_rota_de_chamada(callee, argc, || declarada)
    }

    /// As três identidades que permanecem intencionalmente fora do subset nativo.
    const EXCLUSOES_STDIN: [&str; 3] = ["ouvir", "ouvir_verso", "ouvir_verso_ou"];

    // --- F3: as cinco rotas reparadas resolvem pela autoridade esperada ---

    #[test]
    fn as_cinco_rotas_reparadas_resolvem_para_o_simbolo_de_runtime_esperado() {
        let esperado: [(&str, usize, &str); 6] = [
            ("afirmar", 1, "pinker_afirmar_1"),
            ("afirmar", 2, "pinker_afirmar_2"),
            ("dormir", 1, "pinker_dormir"),
            (
                "emitir_linha_csv_bombom",
                2,
                "pinker_emitir_linha_csv_bombom",
            ),
            ("ler_linha_csv_bombom", 2, "pinker_ler_linha_csv_bombom"),
            ("sair", 1, "pinker_sair"),
        ];
        for (callee, argc, simbolo) in esperado {
            assert_eq!(
                rota(callee, argc, false),
                RotaDeChamada::Runtime(simbolo.to_string()),
                "rota de {callee}/{argc}"
            );
        }
    }

    #[test]
    fn afirmar_despacha_por_aridade_e_recusa_fora_do_recorte() {
        assert_eq!(
            rota("afirmar", 1, false),
            RotaDeChamada::Runtime("pinker_afirmar_1".to_string())
        );
        assert_eq!(
            rota("afirmar", 2, false),
            RotaDeChamada::Runtime("pinker_afirmar_2".to_string())
        );
        for argc in [0, 3, 4] {
            assert_eq!(
                rota("afirmar", argc, false),
                RotaDeChamada::AridadeForaDoRecorte,
                "afirmar/{argc} deve ser recusada"
            );
        }
    }

    #[test]
    fn funcao_pinker_ordinaria_nao_e_capturada_pelas_rotas_nativas() {
        for callee in ["afirmar_usuario", "dormir_bem", "sair_do_laco", "csv_meu"] {
            assert_eq!(
                rota(callee, 1, true),
                RotaDeChamada::FuncaoPinker(callee.to_string()),
                "{callee} deve permanecer função Pinker"
            );
        }
    }

    #[test]
    fn callee_desconhecido_continua_rejeitado() {
        for callee in ["inexistente_522", "afirmar_usuario", "ouvir"] {
            assert_eq!(
                rota(callee, 1, false),
                RotaDeChamada::CalleeDesconhecido,
                "{callee} sem declaração deve ser recusado"
            );
        }
    }

    #[test]
    fn precedencia_de_autoridade_e_estavel_entre_call_e_callvoid() {
        // A precedência é: aridade, depois nome, depois função Pinker declarada.
        // Uma intrínseca reconhecida não deixa de sê-lo porque existe função
        // homônima declarada; a invalidez dessa declaração é decidida antes,
        // na semântica (#502), e não é relaxada aqui.
        assert_eq!(
            rota("afirmar", 1, true),
            RotaDeChamada::Runtime("pinker_afirmar_1".to_string())
        );
        // Símbolo por nome não é sensível à aridade do call site.
        for argc in [0, 1, 2, 3] {
            assert_eq!(
                rota("sair", argc, false),
                RotaDeChamada::Runtime("pinker_sair".to_string()),
                "sair/{argc}"
            );
        }
    }

    // --- F4: ausência estrutural das exclusões de stdin em TODAS as autoridades ---

    /// Predicado estrutural: nenhuma das três exclusões participa de qualquer
    /// rota nativa alcançável pelo backend, sob o despacho fornecido.
    fn exclusoes_ausentes_de_todas_as_autoridades(
        elegivel_por_aridade: impl Fn(&str) -> bool,
        simbolo_por_aridade: impl Fn(&str, usize) -> Option<String>,
        simbolo_por_nome: impl Fn(&str) -> Option<&'static str>,
    ) -> bool {
        EXCLUSOES_STDIN.iter().all(|callee| {
            !elegivel_por_aridade(callee)
                && (0..=3).all(|argc| simbolo_por_aridade(callee, argc).is_none())
                && simbolo_por_nome(callee).is_none()
        })
    }

    #[test]
    fn exclusoes_de_stdin_ausentes_de_todas_as_autoridades_de_despacho() {
        assert!(
            exclusoes_ausentes_de_todas_as_autoridades(
                is_arity_runtime_intrinsic,
                runtime_intrinsic_symbol_por_aridade,
                runtime_intrinsic_symbol,
            ),
            "ouvir/ouvir_verso/ouvir_verso_ou não podem participar de nenhuma rota nativa"
        );
        // E a decisão composta também as recusa.
        for callee in EXCLUSOES_STDIN {
            assert_eq!(rota(callee, 0, false), RotaDeChamada::CalleeDesconhecido);
            assert_eq!(rota(callee, 1, false), RotaDeChamada::CalleeDesconhecido);
        }
    }

    #[test]
    fn probe_de_sensibilidade_detecta_ouvir_introduzida_no_despacho_por_aridade() {
        // Mutação reversível, local ao teste: um despacho que passa a
        // reconhecer `ouvir` por aridade. O predicado estrutural deve ficar
        // vermelho, provando que ele realmente cobre essa autoridade.
        let mutado_elegivel =
            |callee: &str| callee == "ouvir" || is_arity_runtime_intrinsic(callee);
        let mutado_por_aridade = |callee: &str, argc: usize| {
            if callee == "ouvir" && argc == 0 {
                return Some("pinker_ouvir".to_string());
            }
            runtime_intrinsic_symbol_por_aridade(callee, argc)
        };
        assert!(
            !exclusoes_ausentes_de_todas_as_autoridades(
                mutado_elegivel,
                mutado_por_aridade,
                runtime_intrinsic_symbol,
            ),
            "o predicado precisa ficar vermelho quando ouvir entra pelo despacho por aridade"
        );
    }

    #[test]
    fn probe_de_sensibilidade_detecta_ouvir_introduzida_no_despacho_por_nome() {
        let mutado_por_nome = |callee: &str| -> Option<&'static str> {
            if callee == "ouvir_verso" {
                return Some("pinker_ouvir_verso");
            }
            runtime_intrinsic_symbol(callee)
        };
        assert!(
            !exclusoes_ausentes_de_todas_as_autoridades(
                is_arity_runtime_intrinsic,
                runtime_intrinsic_symbol_por_aridade,
                mutado_por_nome,
            ),
            "o predicado precisa ficar vermelho quando ouvir entra pelo despacho por nome"
        );
    }

    // --- F3: o conjunto reconhecido não é ampliado por acidente ---

    #[test]
    fn conjunto_por_aridade_e_exatamente_o_recorte_autorizado() {
        // Fecha o conjunto nominal do despacho por aridade. Ampliá-lo sem
        // atualizar esta tabela deixa o teste vermelho.
        let nominais_esperados = [
            "afirmar",
            "executar_processo",
            "capturar_stdout",
            "capturar_stderr",
            "executar_com_entrada",
        ];
        for callee in nominais_esperados {
            assert!(
                is_arity_runtime_intrinsic(callee),
                "{callee} deveria ser elegível"
            );
        }
        for callee in [
            "dormir",
            "sair",
            "emitir_linha_csv_bombom",
            "ler_linha_csv_bombom",
        ] {
            assert!(
                !is_arity_runtime_intrinsic(callee),
                "{callee} resolve por nome, não por aridade"
            );
        }
        for callee in EXCLUSOES_STDIN {
            assert!(!is_arity_runtime_intrinsic(callee), "{callee} é exclusão");
        }
    }
}
// @pinker-nav:end evidencia.backend-s.selecao-de-rota-nativa
