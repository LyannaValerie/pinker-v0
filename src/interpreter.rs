//! Interpretador da Machine abstrata de pilha do Pinker.
//!
//! Executa um `MachineProgram` validado chamando `principal` com frame
//! próprio de slots e pilha de operandos. Suporta chamadas entre funções,
//! recursão, globals literais e stack trace simples em erros de runtime.
//!
//! Ponto de entrada: [`run_program`].

use crate::abstract_machine::{
    MachineFunction, MachineGlobal, MachineInstr, MachineProgram, MachineTerminator,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::token::Span;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_CALL_DEPTH: usize = 64;

#[derive(Clone)]
struct UnionRuntimeDescriptor {
    union_type_id: crate::ir::UnionTypeId,
    tag: u64,
    payload: RuntimeValue,
    payload_size: u64,
    payload_align: u64,
}

struct UnionRuntimeState {
    next_handle: usize,
    descriptors: HashMap<usize, UnionRuntimeDescriptor>,
}

impl Default for UnionRuntimeState {
    fn default() -> Self {
        Self {
            next_handle: 0x7000_0000,
            descriptors: HashMap::new(),
        }
    }
}

thread_local! {
    static UNION_RUNTIME_STATE: RefCell<UnionRuntimeState> =
        RefCell::new(UnionRuntimeState::default());
}

// Truncamento de stack trace longo (Fase 27b):
// traces com mais de TRACE_TRUNC_THRESHOLD frames são resumidos mostrando
// os primeiros TRACE_HEAD e os últimos TRACE_TAIL, com linha de omissão.
const TRACE_TRUNC_THRESHOLD: usize = 10;
const TRACE_HEAD: usize = 5;
const TRACE_TAIL: usize = 5;

// @pinker-nav:start interpreter.modelo.valores-estado
// @pinker-nav:domain modelo
// @pinker-nav:layer interpreter
// @pinker-nav:summary Define valores executados, handles lógicos e estados hospedados do interpretador para IO, listas, mapas, leques, aleatoriedade, arquivos e frames de diagnóstico; diferencia slots e endereços simulados de ponteiros nativos e não define a representação do runtime nativo linkável.
enum IntrinsicCall {
    NotIntrinsic,
    Done(Option<RuntimeValue>),
}

enum NamedArgLookup<'a> {
    Missing,
    PresentWithoutValue,
    PresentValue(&'a str),
}

struct RuntimeIoState {
    open_files: HashMap<u64, RuntimeOpenFile>,
    next_file_handle: u64,
    closed_handles: std::collections::HashSet<u64>,
    cli_args: Vec<String>,
    exit_status: Option<i32>,
}

struct RuntimeListState {
    lists_bombom: HashMap<u64, Vec<u64>>,
    lists_verso: HashMap<u64, Vec<String>>,
    next_list_handle: u64,
}

// Fase 242/243: registro de valores callable — handle de 1 palavra para um
// descritor {nome da função (na Fase 243, o wrapper `__fnref_env_*` para
// referências a função top-level), endereço do ambiente em `memory` quando
// capturante}. `env_addr: None` == `env_ptr` nulo (não-capturante) — mesmo
// sentinela do backend nativo.
struct CallableDescriptor {
    function_name: String,
    env_addr: Option<usize>,
}

struct CallableState {
    table: HashMap<u64, CallableDescriptor>,
    next_handle: u64,
    // Memoiza o handle de cada função top-level não capturante referenciada
    // como valor, para não recriar descritor a cada `PushFunctionRef`.
    static_by_name: HashMap<String, u64>,
    // Fase 243: contador de endereços simulados para ambientes de closure em
    // heap. Base bem acima de qualquer endereço estático (`build_memory`
    // começa em 1) para nunca colidir.
    next_heap_addr: usize,
}

impl CallableState {
    fn new() -> Self {
        CallableState {
            table: HashMap::new(),
            next_handle: 1,
            static_by_name: HashMap::new(),
            next_heap_addr: 0x1000_0000,
        }
    }

    fn get_or_create_static(&mut self, function_name: &str) -> u64 {
        if let Some(&handle) = self.static_by_name.get(function_name) {
            return handle;
        }
        let handle = self.next_handle;
        self.next_handle += 1;
        self.table.insert(
            handle,
            CallableDescriptor {
                function_name: function_name.to_string(),
                env_addr: None,
            },
        );
        self.static_by_name
            .insert(function_name.to_string(), handle);
        handle
    }

    // Fase 243: cria uma NOVA instância de closure — nunca memoizada, ao
    // contrário de `get_or_create_static` — pois cada criação (cada execução
    // do literal `carinho`) tem seu próprio ambiente, mesmo para o mesmo
    // `function_name` (duas chamadas de `fabricar_somador` produzem duas
    // closures com o mesmo código e ambientes distintos).
    fn create_closure_instance(&mut self, function_name: &str, env_addr: Option<usize>) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.table.insert(
            handle,
            CallableDescriptor {
                function_name: function_name.to_string(),
                env_addr,
            },
        );
        handle
    }

    // Fase 243: reserva `count` endereços contíguos em `memory` para um novo
    // ambiente de closure; devolve o endereço base (offset 0 == captura 0),
    // igual ao layout `env_ptr + i*8` do IR/backend nativo.
    fn allocate_env(&mut self, count: usize) -> usize {
        let base = self.next_heap_addr;
        self.next_heap_addr += count.max(1) * 8;
        base
    }
}

// Fase 244: o objeto público continua sendo apenas um handle de uma palavra.
// O handle indexa `TraitObjectState.table`; o descritor contém o endereço
// simulado do snapshot e um handle separado para uma vtable imutável.
#[derive(Debug, Clone)]
struct TraitObjectDescriptor {
    data_addr: usize,
    vtable_handle: u64,
    concrete_type: crate::ir::TypeIR,
}

#[derive(Debug, Clone)]
struct TraitVtableDescriptor {
    trait_name: String,
    concrete_type_name: String,
    methods: Vec<String>,
}

struct TraitObjectState {
    table: HashMap<u64, TraitObjectDescriptor>,
    vtables: HashMap<u64, TraitVtableDescriptor>,
    vtable_by_key: HashMap<String, u64>,
    next_handle: u64,
    next_vtable_handle: u64,
    next_data_addr: usize,
}

impl TraitObjectState {
    fn new() -> Self {
        TraitObjectState {
            table: HashMap::new(),
            vtables: HashMap::new(),
            vtable_by_key: HashMap::new(),
            next_handle: 1,
            next_vtable_handle: 1,
            // Região distinta dos ambientes de closure, iniciados em
            // 0x1000_0000. Objetos e snapshots vivem por todo o processo.
            next_data_addr: 0x2000_0000,
        }
    }

    fn intern_vtable(
        &mut self,
        trait_name: &str,
        concrete_type_name: &str,
        methods: &[String],
    ) -> u64 {
        let key = format!(
            "{}\u{1f}{}\u{1f}{}",
            trait_name,
            concrete_type_name,
            methods.join("\u{1e}")
        );

        if let Some(handle) = self.vtable_by_key.get(&key) {
            return *handle;
        }

        let handle = self.next_vtable_handle;
        self.next_vtable_handle = self
            .next_vtable_handle
            .checked_add(1)
            .expect("overflow de handles de vtable");

        self.vtables.insert(
            handle,
            TraitVtableDescriptor {
                trait_name: trait_name.to_string(),
                concrete_type_name: concrete_type_name.to_string(),
                methods: methods.to_vec(),
            },
        );
        self.vtable_by_key.insert(key, handle);

        handle
    }

    fn allocate_snapshot(
        &mut self,
        value: RuntimeValue,
        concrete_type: crate::ir::TypeIR,
        concrete_size: u64,
        memory: &mut HashMap<usize, RuntimeValue>,
    ) -> Result<usize, PinkerError> {
        let snapshot_size = usize::try_from(concrete_size)
            .map_err(|_| runtime_err("snapshot de objeto de trato excede o espaço de endereços"))?;

        if snapshot_size == 0 {
            return Err(runtime_err(
                "snapshot de objeto de trato não pode ter tamanho zero",
            ));
        }

        let aligned_size = snapshot_size
            .checked_add(7)
            .map(|size| size & !7usize)
            .ok_or_else(|| runtime_err("overflow ao alinhar snapshot de objeto de trato"))?;

        let base = self.next_data_addr;
        self.next_data_addr = self
            .next_data_addr
            .checked_add(aligned_size.max(8))
            .ok_or_else(|| runtime_err("espaço de snapshots de objetos de trato esgotado"))?;

        if matches!(
            concrete_type,
            crate::ir::TypeIR::Struct | crate::ir::TypeIR::FixedArray { .. }
        ) {
            let RuntimeValue::Ptr(source_addr) = value else {
                return Err(runtime_err(
                    "snapshot composto de objeto de trato exige endereço de origem",
                ));
            };

            let source_end = source_addr
                .checked_add(snapshot_size)
                .ok_or_else(|| runtime_err("overflow ao delimitar snapshot composto"))?;

            // Copia todas as células existentes no intervalo real do valor,
            // preservando offsets e padding. Ponteiros internos continuam
            // sendo valores de ponteiro, como numa cópia byte a byte.
            let cells = memory
                .iter()
                .filter_map(|(addr, cell)| {
                    if *addr >= source_addr && *addr < source_end {
                        Some((*addr - source_addr, cell.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if cells.is_empty() {
                return Err(runtime_err(
                    "snapshot composto parte de endereço inválido ou não inicializado",
                ));
            }

            for (offset, cell) in cells {
                memory.insert(base + offset, cell);
            }
        } else {
            // Primitivos, versos, ponteiros, handles hospedados e callables
            // ocupam uma célula lógica. `clone` congela o valor no momento da
            // materialização e elimina dependência do slot de origem.
            memory.insert(base, value);
        }

        Ok(base)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_object(
        &mut self,
        value: RuntimeValue,
        trait_name: &str,
        concrete_type: crate::ir::TypeIR,
        concrete_type_name: &str,
        concrete_size: u64,
        vtable_methods: &[String],
        memory: &mut HashMap<usize, RuntimeValue>,
    ) -> Result<u64, PinkerError> {
        let vtable_handle = self.intern_vtable(trait_name, concrete_type_name, vtable_methods);

        let data_addr = self.allocate_snapshot(value, concrete_type, concrete_size, memory)?;

        let handle = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .ok_or_else(|| runtime_err("espaço de handles de objetos de trato esgotado"))?;

        self.table.insert(
            handle,
            TraitObjectDescriptor {
                data_addr,
                vtable_handle,
                concrete_type,
            },
        );

        Ok(handle)
    }

    fn resolve_call(
        &self,
        handle: u64,
        trait_name: &str,
        method_name: &str,
        method_slot: u64,
        memory: &HashMap<usize, RuntimeValue>,
    ) -> Result<(String, RuntimeValue), PinkerError> {
        let descriptor = self
            .table
            .get(&handle)
            .cloned()
            .ok_or_else(|| runtime_err("trait_call com handle de objeto de trato inválido"))?;

        let vtable = self
            .vtables
            .get(&descriptor.vtable_handle)
            .cloned()
            .ok_or_else(|| runtime_err("trait_call com vtable inexistente"))?;

        if vtable.trait_name != trait_name {
            return Err(runtime_err(&format!(
                "trait_call de trato incompatível: objeto é trato<{}>, chamada exige trato<{}>",
                vtable.trait_name, trait_name
            )));
        }

        let slot = usize::try_from(method_slot)
            .map_err(|_| runtime_err("trait_call com slot de vtable fora da faixa"))?;

        let function_name = vtable.methods.get(slot).cloned().ok_or_else(|| {
            runtime_err(&format!(
                "trait_call com slot de vtable inválido para {}",
                vtable.concrete_type_name
            ))
        })?;

        let expected_suffix = format!("_{}", method_name);

        if !function_name.ends_with(&expected_suffix) {
            return Err(runtime_err(
                "trait_call encontrou método divergente no slot da vtable",
            ));
        }

        let receiver = if matches!(
            descriptor.concrete_type,
            crate::ir::TypeIR::Struct | crate::ir::TypeIR::FixedArray { .. }
        ) {
            RuntimeValue::Ptr(descriptor.data_addr)
        } else {
            memory
                .get(&descriptor.data_addr)
                .cloned()
                .ok_or_else(|| runtime_err("snapshot de objeto de trato ausente"))?
        };

        Ok((function_name, receiver))
    }
}

struct RuntimeMapState {
    maps_verso_bombom: HashMap<u64, HashMap<String, u64>>,
    maps_verso_verso: HashMap<u64, HashMap<String, String>>,
    maps_bombom_bombom: HashMap<u64, HashMap<u64, u64>>,
    maps_bombom_verso: HashMap<u64, HashMap<u64, String>>,
    next_map_handle: u64,
    map_iters_verso_bombom: HashMap<u64, RuntimeMapVersoBombomIter>,
    map_iters_verso_verso: HashMap<u64, RuntimeMapVersoVersoIter>,
    map_iters_bombom_bombom: HashMap<u64, RuntimeMapBombomBombomIter>,
    map_iters_bombom_verso: HashMap<u64, RuntimeMapBombomVersoIter>,
    next_map_iter_handle: u64,
    // Fases 209–210 — valores de leque com carga: handle -> (tag, cargas).
    enum_values: HashMap<u64, (u64, Vec<RuntimeEnumPayload>)>,
    next_enum_handle: u64,
}

enum RuntimeEnumPayload {
    Int(u64),
    Str(String),
}

struct RuntimeRandomState {
    generators: HashMap<u64, RuntimeRandomGenerator>,
    next_generator_handle: u64,
}

struct RuntimeRandomGenerator {
    state: u64,
}

struct RuntimeMapVersoBombomIter {
    keys_snapshot: Vec<String>,
    next_index: usize,
}

struct RuntimeMapVersoVersoIter {
    keys_snapshot: Vec<String>,
    next_index: usize,
}

struct RuntimeMapBombomBombomIter {
    keys_snapshot: Vec<u64>,
    next_index: usize,
}

struct RuntimeMapBombomVersoIter {
    keys_snapshot: Vec<u64>,
    next_index: usize,
}

struct RuntimeOpenFile {
    path: String,
    content: String,
    append_enabled: bool,
}

#[derive(Debug, Clone)]
struct RuntimeFrame {
    fn_name: String,
    block_label: Option<String>,
    current_instr: Option<&'static str>,
    future_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeValue {
    Int(u64),
    IntSigned(i64),
    Ptr(usize),
    Bool(bool),
    Str(String),
    ListBombom(u64),
    ListVerso(u64),
    MapVersoBombom(u64),
    MapVersoVerso(u64),
    MapBombomBombom(u64),
    MapBombomVerso(u64),
    // Fase 242: handle callable — índice em `CallableState.table`, mesmo
    // padrão de handle já usado por `ListBombom`/`enum_values`.
    Callable(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub return_value: Option<RuntimeValue>,
    pub exit_status: Option<i32>,
}

// @pinker-nav:end interpreter.modelo.valores-estado

// @pinker-nav:start interpreter.execucao.programa-globais
// @pinker-nav:domain execucao
// @pinker-nav:layer interpreter
// @pinker-nav:summary Inicia a execução hospedada de um `MachineProgram`, copia argumentos CLI para o estado do interpretador, chama `principal`, converte globais em `RuntimeValue`, monta a memória indireta simulada em `HashMap` e devolve valor ou status de saída sem gerar código nativo.
pub fn run_program(program: &MachineProgram) -> Result<Option<RuntimeValue>, PinkerError> {
    Ok(run_program_with_args(program, &[])?.return_value)
}

pub fn run_program_with_args(
    program: &MachineProgram,
    cli_args: &[String],
) -> Result<RunOutcome, PinkerError> {
    UNION_RUNTIME_STATE.with(|state| *state.borrow_mut() = UnionRuntimeState::default());
    let globals = build_globals(program)?;
    let mut memory = build_memory(program, &globals)?;
    let mut io_state = RuntimeIoState {
        open_files: HashMap::new(),
        next_file_handle: 1,
        closed_handles: std::collections::HashSet::new(),
        cli_args: cli_args.to_vec(),
        exit_status: None,
    };
    let mut list_state = RuntimeListState {
        lists_bombom: HashMap::new(),
        lists_verso: HashMap::new(),
        next_list_handle: 1,
    };
    let mut map_state = RuntimeMapState {
        maps_verso_bombom: HashMap::new(),
        maps_verso_verso: HashMap::new(),
        maps_bombom_bombom: HashMap::new(),
        maps_bombom_verso: HashMap::new(),
        next_map_handle: 1,
        map_iters_verso_bombom: HashMap::new(),
        map_iters_verso_verso: HashMap::new(),
        map_iters_bombom_bombom: HashMap::new(),
        map_iters_bombom_verso: HashMap::new(),
        next_map_iter_handle: 1,
        enum_values: HashMap::new(),
        next_enum_handle: 1,
    };
    let mut random_state = RuntimeRandomState {
        generators: HashMap::new(),
        next_generator_handle: 1,
    };
    let mut public_memory_state = PublicMemoryState::default();
    let mut callable_state = CallableState::new();
    let mut trait_object_state = TraitObjectState::new();
    let mut call_stack = Vec::new();
    let return_value = call_function(
        "principal",
        vec![],
        program,
        &globals,
        &mut memory,
        &mut public_memory_state,
        &mut io_state,
        &mut list_state,
        &mut map_state,
        &mut random_state,
        &mut callable_state,
        &mut trait_object_state,
        &mut call_stack,
    )?;
    let principal_status = match &return_value {
        Some(RuntimeValue::Int(value)) => Some((value & 0xff) as i32),
        Some(RuntimeValue::IntSigned(value)) => Some(((*value as u64) & 0xff) as i32),
        Some(RuntimeValue::Ptr(value)) => Some(((*value as u64) & 0xff) as i32),
        _ => None,
    };
    Ok(RunOutcome {
        return_value,
        exit_status: io_state.exit_status.or(principal_status),
    })
}

fn build_globals(program: &MachineProgram) -> Result<HashMap<String, RuntimeValue>, PinkerError> {
    let mut globals = HashMap::new();
    for g in &program.globals {
        let value = eval_global_value(g)?;
        globals.insert(g.name.clone(), value);
    }
    Ok(globals)
}

fn eval_global_value(g: &MachineGlobal) -> Result<RuntimeValue, PinkerError> {
    match (&g.value, g.ty) {
        (OperandIR::Int(v), crate::ir::TypeIR::Pointer { .. }) => {
            Ok(RuntimeValue::Ptr(*v as usize))
        }
        (OperandIR::Int(v), ty) if ty.is_integer() => {
            coerce_runtime_value_to_type(RuntimeValue::Int(*v), ty)
        }
        (OperandIR::Bool(v), _) => Ok(RuntimeValue::Bool(*v)),
        (OperandIR::Str(s), _) => Ok(RuntimeValue::Str(s.clone())),
        _ => Err(runtime_err("valor global não suportado em runtime")),
    }
}

fn build_memory(
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
) -> Result<HashMap<usize, RuntimeValue>, PinkerError> {
    let mut memory = HashMap::new();
    let mut next_addr: usize = 1;
    for g in &program.globals {
        match g.ty {
            crate::ir::TypeIR::Bombom
            | crate::ir::TypeIR::U8
            | crate::ir::TypeIR::U16
            | crate::ir::TypeIR::U32
            | crate::ir::TypeIR::U64
            | crate::ir::TypeIR::I8
            | crate::ir::TypeIR::I16
            | crate::ir::TypeIR::I32
            | crate::ir::TypeIR::I64
            | crate::ir::TypeIR::Logica => {
                let value = globals
                    .get(&g.name)
                    .cloned()
                    .ok_or_else(|| runtime_err("global inexistente em runtime"))?;
                memory.insert(next_addr, value);
                next_addr = next_addr.saturating_add(1);
            }
            _ => {}
        }
    }
    Ok(memory)
}

// Executa uma função pelo nome com os argumentos fornecidos.
// O call_stack acumula os nomes ativos para montar o stack trace em erros.
// Retorna `None` para funções void, `Some(valor)` caso contrário.
#[allow(clippy::too_many_arguments)]
// @pinker-nav:end interpreter.execucao.programa-globais

// @pinker-nav:start interpreter.execucao.funcoes-fluxo
// @pinker-nav:domain execucao
// @pinker-nav:layer interpreter
// @pinker-nav:summary Executa uma `MachineFunction` validada a partir do bloco `entry`, criando frame, slots, pilha e mapa de labels, seguindo terminadores e propagando retornos ou `sair`; consulta intrínsecas hospedadas e funções Pinker sem reconstruir CFG, escalonar concorrência ou emitir ABI nativa.
fn call_function(
    fn_name: &str,
    args: Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    public_memory_state: &mut PublicMemoryState,
    io_state: &mut RuntimeIoState,
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
    callable_state: &mut CallableState,
    trait_object_state: &mut TraitObjectState,
    call_stack: &mut Vec<RuntimeFrame>,
) -> Result<Option<RuntimeValue>, PinkerError> {
    if call_stack.len() >= MAX_CALL_DEPTH {
        return Err(runtime_err(&format!(
            "limite preventivo de recursão excedido: profundidade máxima de chamadas ({MAX_CALL_DEPTH}) atingida ao entrar em '{fn_name}'"
        )));
    }

    call_stack.push(RuntimeFrame {
        fn_name: fn_name.to_string(),
        block_label: None,
        current_instr: None,
        future_span: None,
    });

    // Encapsula a execução numa closure para poder anexar o trace no retorno.
    let result = (|| {
        let function = find_function(fn_name, program)?;

        if function.params.len() != args.len() {
            return Err(runtime_err(&format!(
                "[{}] chamada com aridade inválida",
                fn_name
            )));
        }

        let mut labels = HashMap::new();
        for (idx, block) in function.blocks.iter().enumerate() {
            labels.insert(block.label.clone(), idx);
        }

        let mut slots: HashMap<String, RuntimeValue> = HashMap::new();
        for (slot, value) in function.params.iter().cloned().zip(args.into_iter()) {
            let coerced = if let Some(ty) = function.slot_types.get(&slot) {
                coerce_runtime_value_to_type(value, *ty)?
            } else {
                value
            };
            slots.insert(slot, coerced);
        }

        let mut stack: Vec<RuntimeValue> = Vec::new();
        let mut current_label = "entry".to_string();

        loop {
            let Some(&block_idx) = labels.get(&current_label) else {
                return Err(runtime_err(&format!(
                    "[{}] label de execução inexistente: {}",
                    fn_name, current_label
                )));
            };
            let block = &function.blocks[block_idx];
            if let Some(frame) = call_stack.last_mut() {
                frame.block_label = Some(block.label.clone());
            }

            for instr in &block.code {
                set_current_instr(call_stack, Some(machine_instr_name(instr)));
                exec_instr(
                    instr,
                    &mut slots,
                    &mut stack,
                    program,
                    globals,
                    memory,
                    public_memory_state,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    callable_state,
                    trait_object_state,
                    call_stack,
                )?;
                set_current_instr(call_stack, None);
                if io_state.exit_status.is_some() {
                    return Ok(None);
                }
            }

            match &block.terminator {
                MachineTerminator::Jmp(target) => {
                    current_label.clone_from(target);
                }
                MachineTerminator::BrTrue {
                    then_label,
                    else_label,
                } => {
                    let cond = pop_bool(&mut stack, "br_true requer bool no topo")?;
                    current_label = if cond {
                        then_label.clone()
                    } else {
                        else_label.clone()
                    };
                }
                MachineTerminator::Ret => {
                    if stack.len() != 1 {
                        return Err(runtime_err(&format!(
                            "[{}] ret inválido: pilha deve ter 1 valor",
                            fn_name
                        )));
                    }
                    let value = stack.pop().expect("len checked");
                    return Ok(Some(coerce_runtime_value_to_type(
                        value,
                        function.ret_type,
                    )?));
                }
                MachineTerminator::RetVoid => {
                    if !stack.is_empty() {
                        return Err(runtime_err(&format!(
                            "[{}] ret_void inválido: pilha deve estar vazia",
                            fn_name
                        )));
                    }
                    return Ok(None);
                }
            }
        }
    })();

    let result = result.map_err(|err| attach_runtime_trace(err, call_stack));
    let _ = call_stack.pop();
    result
}

#[allow(clippy::too_many_arguments)]
// @pinker-nav:end interpreter.execucao.funcoes-fluxo

// @pinker-nav:start interpreter.execucao.instrucoes-pilha
// @pinker-nav:domain execucao
// @pinker-nav:layer interpreter
// @pinker-nav:summary Executa instruções da máquina de pilha lendo ou desempilhando operandos, mutando slots, pilha, globais e memória simulada, despachando intrínsecas antes de funções Pinker e materializando impressões de `falar`; mantém verificações defensivas de underflow e tipos sem substituir a validação estática.
const RAW_FUNCTION_ADDRESS_BASE: usize = 0x7000_0000;

fn raw_function_address(program: &MachineProgram, name: &str) -> Option<usize> {
    program
        .functions
        .iter()
        .position(|function| function.name == name)
        .map(|index| RAW_FUNCTION_ADDRESS_BASE + index * 8)
}

fn raw_function_name(program: &MachineProgram, address: usize) -> Option<&str> {
    let offset = address.checked_sub(RAW_FUNCTION_ADDRESS_BASE)?;
    if offset % 8 != 0 {
        return None;
    }
    program
        .functions
        .get(offset / 8)
        .map(|function| function.name.as_str())
}

#[allow(clippy::too_many_arguments)]
fn exec_instr(
    instr: &MachineInstr,
    slots: &mut HashMap<String, RuntimeValue>,
    stack: &mut Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    public_memory_state: &mut PublicMemoryState,
    io_state: &mut RuntimeIoState,
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
    callable_state: &mut CallableState,
    trait_object_state: &mut TraitObjectState,
    call_stack: &mut Vec<RuntimeFrame>,
) -> Result<(), PinkerError> {
    match instr {
        MachineInstr::PushInt(v) => stack.push(RuntimeValue::Int(*v)),
        MachineInstr::PushBool(v) => stack.push(RuntimeValue::Bool(*v)),
        MachineInstr::PushStr(v) => stack.push(RuntimeValue::Str(v.clone())),
        MachineInstr::LoadSlot(slot) => {
            let Some(value) = slots.get(slot).cloned() else {
                return Err(runtime_err("load_slot em slot não inicializado"));
            };
            stack.push(value);
        }
        MachineInstr::LoadGlobal(name) => {
            let Some(value) = globals.get(name).cloned() else {
                return Err(runtime_err("global inexistente em runtime"));
            };
            stack.push(value);
        }
        MachineInstr::StoreSlot(slot) => {
            let value = pop(stack, "store_slot exige valor na pilha")?;
            let coerced =
                if let Some(ty) = current_function(program, call_stack)?.slot_types.get(slot) {
                    coerce_runtime_value_to_type(value, *ty)?
                } else {
                    value
                };
            slots.insert(slot.clone(), coerced);
        }
        MachineInstr::Neg { ty } => {
            let value = pop_numeric(stack, "neg exige inteiro no topo")?;
            let out = match value {
                RuntimeValue::Int(v) => RuntimeValue::Int((0u64).wrapping_sub(v)),
                RuntimeValue::IntSigned(v) => RuntimeValue::IntSigned(v.wrapping_neg()),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(normalize_integer(out, *ty)?);
        }
        MachineInstr::Not => {
            let value = pop_bool(stack, "not exige lógica no topo")?;
            stack.push(RuntimeValue::Bool(!value));
        }
        MachineInstr::BitNot { ty } => {
            let value = pop_numeric(stack, "bitnot exige inteiro no topo")?;
            let out = match value {
                RuntimeValue::Int(v) => RuntimeValue::Int(!v),
                RuntimeValue::IntSigned(v) => RuntimeValue::IntSigned(!v),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(normalize_integer(out, *ty)?);
        }
        MachineInstr::DerefLoad {
            ty, is_volatile, ..
        } => {
            let ptr = pop(stack, "deref_load exige ponteiro no topo")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err("deref_load exige ponteiro no topo"));
            };
            if matches!(ty, crate::ir::TypeIR::FixedArray { .. }) {
                stack.push(RuntimeValue::Ptr(addr));
                return Ok(());
            }
            let width = runtime_type_width(*ty);
            let public_region = public_memory_access_region(public_memory_state, addr, width)?;
            if let Some((base, size, alive)) = public_region {
                if !alive {
                    return Err(runtime_err(
                        "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública",
                    ));
                }
                if addr % width != 0 {
                    return Err(runtime_err(
                        "E-RUNTIME-MEM-MISALIGNED: acesso desalinhado à memória pública",
                    ));
                }
                if !public_memory_interval_contained(base, size, addr, width)? {
                    let message = if addr >= base {
                        "E-RUNTIME-MEM-CROSS-BOUNDARY: acesso multibyte cruza o limite da alocação pública"
                    } else {
                        "E-RUNTIME-MEM-OUT-OF-BOUNDS: acesso fora dos limites da alocação pública"
                    };
                    return Err(runtime_err(message));
                }
            }
            let loaded = if public_region.is_some() {
                Some(public_memory_load_bytes(
                    &public_memory_state.payload,
                    addr,
                    *ty,
                )?)
            } else if *is_volatile {
                deref_load_fragil(memory, addr)
            } else {
                deref_load_normal(memory, addr)
            };
            let Some(value) = loaded else {
                return Err(runtime_err(
                    "deref_load em endereço inválido ou não inicializado",
                ));
            };
            stack.push(value);
        }
        MachineInstr::DerefStore { ty, is_volatile } => {
            let value = pop(stack, "deref_store exige valor no topo")?;
            let ptr = pop(stack, "deref_store exige ponteiro abaixo do valor")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err(
                    "deref_store exige ponteiro abaixo do valor no topo",
                ));
            };
            let width = runtime_type_width(*ty);
            let public_region = public_memory_access_region(public_memory_state, addr, width)?;
            if let Some((base, size, alive)) = public_region {
                if !alive {
                    return Err(runtime_err(
                        "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública",
                    ));
                }
                if addr % width != 0 {
                    return Err(runtime_err(
                        "E-RUNTIME-MEM-MISALIGNED: acesso desalinhado à memória pública",
                    ));
                }
                if !public_memory_interval_contained(base, size, addr, width)? {
                    let message = if addr >= base {
                        "E-RUNTIME-MEM-CROSS-BOUNDARY: acesso multibyte cruza o limite da alocação pública"
                    } else {
                        "E-RUNTIME-MEM-OUT-OF-BOUNDS: acesso fora dos limites da alocação pública"
                    };
                    return Err(runtime_err(message));
                }
            }
            if public_region.is_some() {
                let coerced = coerce_runtime_value_to_type(value, *ty)?;
                public_memory_store_bytes(
                    &mut public_memory_state.payload,
                    addr,
                    *ty,
                    coerced,
                )?;
                return Ok(());
            }
            if !memory.contains_key(&addr) {
                return Err(runtime_err(
                    "deref_store em endereço inválido ou não inicializado",
                ));
            }
            let coerced = coerce_runtime_value_to_type(value, *ty)?;
            if *is_volatile {
                deref_store_fragil(memory, addr, coerced);
            } else {
                deref_store_normal(memory, addr, coerced);
            }
        }
        MachineInstr::Cast { ty } => {
            let value = pop(stack, "cast exige valor no topo")?;
            let casted = coerce_runtime_value_to_type(value, *ty)?;
            stack.push(casted);
        }
        MachineInstr::MakeUnion {
            union_type_id,
            tag,
            payload_type,
            payload_size,
            payload_align,
        } => {
            let payload = pop(stack, "make_union exige payload no topo")?;
            let payload = coerce_runtime_value_to_type(payload, *payload_type)?;
            let union = program
                .union_types
                .iter()
                .find(|union| union.id == *union_type_id)
                .ok_or_else(|| runtime_err("tipo de união não registrado"))?;
            let member = union
                .members
                .iter()
                .find(|member| member.tag == *tag)
                .ok_or_else(|| runtime_err("tag de união não registrada"))?;
            if member.ty != *payload_type
                || member.size != *payload_size
                || member.align != *payload_align
            {
                return Err(runtime_err("layout de união divergente no runtime"));
            }
            let handle = UNION_RUNTIME_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let handle = state.next_handle;
                state.next_handle = state
                    .next_handle
                    .checked_add(16)
                    .ok_or_else(|| runtime_err("overflow de handle de união"))?;
                state.descriptors.insert(
                    handle,
                    UnionRuntimeDescriptor {
                        union_type_id: *union_type_id,
                        tag: *tag,
                        payload,
                        payload_size: *payload_size,
                        payload_align: *payload_align,
                    },
                );
                Ok::<usize, PinkerError>(handle)
            })?;
            stack.push(RuntimeValue::Ptr(handle));
        }
        MachineInstr::BitAnd { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitand exige dois inteiros")?;
            stack.push(normalize_integer(
                bin_int(lhs, rhs, |a, b| a & b, |a, b| a & b)?,
                *ty,
            )?);
        }
        MachineInstr::BitOr { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitor exige dois inteiros")?;
            stack.push(normalize_integer(
                bin_int(lhs, rhs, |a, b| a | b, |a, b| a | b)?,
                *ty,
            )?);
        }
        MachineInstr::BitXor { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitxor exige dois inteiros")?;
            stack.push(normalize_integer(
                bin_int(lhs, rhs, |a, b| a ^ b, |a, b| a ^ b)?,
                *ty,
            )?);
        }
        MachineInstr::Shl { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "shl exige dois inteiros")?;
            stack.push(eval_shift(lhs, rhs, *ty, false)?);
        }
        MachineInstr::Shr { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "shr exige dois inteiros")?;
            stack.push(eval_shift(lhs, rhs, *ty, true)?);
        }
        MachineInstr::Add { ty } => {
            let rhs = pop(stack, "underflow em add")?;
            let lhs = pop(stack, "underflow em add")?;
            let origem = match &lhs {
                RuntimeValue::Ptr(base) => Some(*base),
                _ => None,
            };
            let resultado = eval_add(lhs, rhs)?;
            let resultado = if ty.is_integer() {
                normalize_integer(resultado, *ty)?
            } else {
                resultado
            };
            validar_derivacao_memoria_publica(public_memory_state, origem, &resultado)?;
            stack.push(resultado);
        }
        MachineInstr::Sub { ty } => {
            let rhs = pop(stack, "underflow em sub")?;
            let lhs = pop(stack, "underflow em sub")?;
            let origem = match &lhs {
                RuntimeValue::Ptr(base) => Some(*base),
                _ => None,
            };
            let resultado = eval_sub(lhs, rhs)?;
            let resultado = if ty.is_integer() {
                normalize_integer(resultado, *ty)?
            } else {
                resultado
            };
            validar_derivacao_memoria_publica(public_memory_state, origem, &resultado)?;
            stack.push(resultado);
        }
        MachineInstr::Mul { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "mul exige dois inteiros")?;
            stack.push(normalize_integer(
                bin_int(lhs, rhs, |a, b| a.wrapping_mul(b), |a, b| a.wrapping_mul(b))?,
                *ty,
            )?);
        }
        MachineInstr::Div { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "div exige dois inteiros")?;
            stack.push(normalize_integer(bin_int_checked_div(lhs, rhs)?, *ty)?);
        }
        MachineInstr::Mod { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "mod exige dois inteiros")?;
            stack.push(normalize_integer(bin_int_checked_mod(lhs, rhs)?, *ty)?);
        }
        MachineInstr::CmpEq { ty } => {
            let rhs = pop(stack, "cmp_eq exige dois valores")?;
            let lhs = pop(stack, "cmp_eq exige dois valores")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            let equal = match (lhs, rhs) {
                (RuntimeValue::Ptr(a), RuntimeValue::Ptr(b)) => a == b,
                (RuntimeValue::Ptr(a), RuntimeValue::Int(0))
                | (RuntimeValue::Int(0), RuntimeValue::Ptr(a)) => a == 0,
                (lhs, rhs) => cmp_int(lhs, rhs, |a, b| a == b, |a, b| a == b)?,
            };
            stack.push(RuntimeValue::Bool(equal));
        }
        MachineInstr::CmpNe { ty } => {
            let rhs = pop(stack, "cmp_ne exige dois valores")?;
            let lhs = pop(stack, "cmp_ne exige dois valores")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            let different = match (lhs, rhs) {
                (RuntimeValue::Ptr(a), RuntimeValue::Ptr(b)) => a != b,
                (RuntimeValue::Ptr(a), RuntimeValue::Int(0))
                | (RuntimeValue::Int(0), RuntimeValue::Ptr(a)) => a != 0,
                (lhs, rhs) => cmp_int(lhs, rhs, |a, b| a != b, |a, b| a != b)?,
            };
            stack.push(RuntimeValue::Bool(different));
        }
        MachineInstr::CmpLt { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_lt exige dois inteiros")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a < b,
                |a, b| a < b,
            )?));
        }
        MachineInstr::CmpLe { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_le exige dois inteiros")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a <= b,
                |a, b| a <= b,
            )?));
        }
        MachineInstr::CmpGt { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_gt exige dois inteiros")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a > b,
                |a, b| a > b,
            )?));
        }
        MachineInstr::CmpGe { ty } => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_ge exige dois inteiros")?;
            let (lhs, rhs) = normalize_comparison_pair(lhs, rhs, *ty)?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a >= b,
                |a, b| a >= b,
            )?));
        }
        MachineInstr::Call { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = match try_call_intrinsic(
                callee,
                &args,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => call_function(
                    callee,
                    args,
                    program,
                    globals,
                    memory,
                    public_memory_state,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    callable_state,
                    trait_object_state,
                    call_stack,
                )?,
            };
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = match try_call_intrinsic(
                callee,
                &args,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => call_function(
                    callee,
                    args,
                    program,
                    globals,
                    memory,
                    public_memory_state,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    callable_state,
                    trait_object_state,
                    call_stack,
                )?,
            };
            if result.is_some() {
                return Err(runtime_err("call_void exige função sem retorno"));
            }
        }
        MachineInstr::PushFunctionRef(name) => {
            let handle = callable_state.get_or_create_static(name);
            stack.push(RuntimeValue::Callable(handle));
        }
        MachineInstr::PushRawFunctionRef(name) => {
            let address = raw_function_address(program, name).ok_or_else(|| {
                runtime_err("push_raw_function_ref referencia função inexistente")
            })?;
            stack.push(RuntimeValue::Ptr(address));
        }
        MachineInstr::MakeClosure {
            function_name,
            capture_count,
        } => {
            let captured = pop_args(stack, *capture_count)?;
            let env_addr = if captured.is_empty() {
                None
            } else {
                let base = callable_state.allocate_env(captured.len());
                for (index, value) in captured.into_iter().enumerate() {
                    memory.insert(base + index * 8, value);
                }
                Some(base)
            };
            let handle = callable_state.create_closure_instance(function_name, env_addr);
            stack.push(RuntimeValue::Callable(handle));
        }
        MachineInstr::CallIndirect { argc } => {
            let Some(callee_value) = stack.pop() else {
                return Err(runtime_err("call_indirect exige handle callable no topo"));
            };
            let RuntimeValue::Callable(handle) = callee_value else {
                return Err(runtime_err("call_indirect exige valor callable no topo"));
            };
            let user_args = pop_args(stack, *argc)?;
            let Some(descriptor) = callable_state.table.get(&handle) else {
                return Err(runtime_err("call_indirect com handle callable inválido"));
            };
            let function_name = descriptor.function_name.clone();
            // Fase 243: `__env` é sempre o argumento real final (trailing),
            // uniforme para toda função indiretamente chamável — closure ou
            // wrapper de função top-level (`__fnref_env_*`, que o ignora).
            let env_value = RuntimeValue::Ptr(descriptor.env_addr.unwrap_or(0));
            let mut combined_args = user_args;
            combined_args.push(env_value);
            let result = call_function(
                &function_name,
                combined_args,
                program,
                globals,
                memory,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
                callable_state,
                trait_object_state,
                call_stack,
            )?;
            let Some(value) = result else {
                return Err(runtime_err(
                    "call_indirect exige callable com retorno (tipo função público nunca é nulo)",
                ));
            };
            stack.push(value);
        }
        MachineInstr::CallRaw { argc, has_return } => {
            let Some(callee_value) = stack.pop() else {
                return Err(runtime_err("call_raw exige endereço cru no topo"));
            };
            let RuntimeValue::Ptr(address) = callee_value else {
                return Err(runtime_err("call_raw exige ponteiro cru de função"));
            };
            if address == 0 {
                return Err(runtime_err("chamada nula por ponteiro cru de função"));
            }
            let function_name = raw_function_name(program, address)
                .ok_or_else(|| runtime_err("call_raw com endereço de função inválido"))?;
            let args = pop_args(stack, *argc)?;
            let result = call_function(
                function_name,
                args,
                program,
                globals,
                memory,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
                callable_state,
                trait_object_state,
                call_stack,
            )?;
            match (*has_return, result) {
                (true, Some(value)) => stack.push(value),
                (true, None) => {
                    return Err(runtime_err("call_raw esperava retorno com valor"));
                }
                (false, None) => {}
                (false, Some(_)) => {
                    return Err(runtime_err("call_raw nulo recebeu retorno com valor"));
                }
            }
        }
        MachineInstr::MakeTraitObject {
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => {
            let value = pop(stack, "make_trait_object exige valor concreto no topo")?;

            let handle = trait_object_state.create_object(
                value,
                trait_name,
                *concrete_type,
                concrete_type_name,
                *concrete_size,
                vtable_methods,
                memory,
            )?;

            // A representação pública é uma palavra de 64 bits. O tipo
            // estático `TraitObject` impede que esse inteiro seja utilizado
            // como número no programa Pinker.
            stack.push(RuntimeValue::Int(handle));
        }
        MachineInstr::TraitCall {
            trait_name,
            method_name,
            method_slot,
            method_count,
            argc,
            param_types: _,
            ret_type,
        } => {
            if *method_count == 0 || *method_slot >= *method_count {
                return Err(runtime_err("trait_call referencia slot fora da vtable"));
            }
            let object = pop(stack, "trait_call exige handle de objeto no topo")?;

            let RuntimeValue::Int(handle) = object else {
                return Err(runtime_err("trait_call exige handle de objeto de trato"));
            };

            let user_args = pop_args(stack, *argc)?;

            let (function_name, receiver) = trait_object_state.resolve_call(
                handle,
                trait_name,
                method_name,
                *method_slot,
                memory,
            )?;

            // ABI própria: receiver concreto primeiro, seguido somente pelos
            // argumentos públicos. Não existe `__env` e não há CallIndirect.
            let mut combined_args = Vec::with_capacity(user_args.len() + 1);
            combined_args.push(receiver);
            combined_args.extend(user_args);

            let result = call_function(
                &function_name,
                combined_args,
                program,
                globals,
                memory,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
                callable_state,
                trait_object_state,
                call_stack,
            )?;

            if *ret_type == crate::ir::TypeIR::Nulo {
                if result.is_some() {
                    return Err(runtime_err("trait_call nulo recebeu retorno inesperado"));
                }
            } else {
                let Some(value) = result else {
                    return Err(runtime_err("trait_call com retorno recebeu função nulo"));
                };
                stack.push(value);
            }
        }
        MachineInstr::PrintIntInline => {
            match pop_numeric(stack, "print_int_inline exige inteiro no topo")? {
                RuntimeValue::Int(v) => print!("{}", v),
                RuntimeValue::IntSigned(v) => print!("{}", v),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::ListVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapBombomVerso(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
            }
        }
        MachineInstr::PrintBoolInline => {
            let v = pop_bool(stack, "print_bool_inline exige lógica no topo")?;
            print!("{}", if v { "verdade" } else { "falso" });
        }
        MachineInstr::PrintStrValueInline => {
            let s = pop_str(stack, "print_str_value_inline exige verso no topo")?;
            print!("{}", s);
        }
        MachineInstr::PrintStrInline(s) => {
            print!("{}", s);
        }
        MachineInstr::PrintSpace => {
            print!(" ");
        }
        MachineInstr::PrintNewline => {
            println!();
        }
        MachineInstr::InlineAsm { .. } => {
            return Err(runtime_err(
                "E-RUNTIME-SUSSURRO-NATIVO: sussurro exige execução pelo backend nativo x86-64",
            ));
        }
    }

    Ok(())
}

// @pinker-nav:end interpreter.execucao.instrucoes-pilha

// @pinker-nav:start interpreter.intrinsecos.acaso
// @pinker-nav:domain intrinsecos
// @pinker-nav:layer interpreter
// @pinker-nav:summary Implementa intrínsecas hospedadas de aleatoriedade inicial, validando aridade, semente e handle de gerador, mutando o estado pseudoaleatório do interpretador e retornando handles ou números; não representa geradores do runtime nativo.
const PUBLIC_MEMORY_BASE: usize = 0x5000_0000;
const PUBLIC_MEMORY_MAX_IDENTITIES: usize = 1_000_000;
const PUBLIC_MEMORY_MAX_VIRTUAL_BYTES: usize = 8 * 1024 * 1024 * 1024;
const PUBLIC_MEMORY_MAX_METADATA_BYTES: usize =
    PUBLIC_MEMORY_MAX_IDENTITIES * std::mem::size_of::<PublicMemoryRegion>();
const PUBLIC_MEMORY_MAX_QUARANTINE_BYTES: usize = 0;

#[derive(Clone, Debug)]
struct PublicMemoryRegion {
    base: usize,
    size: usize,
    alive: bool,
}

#[derive(Clone, Debug)]
struct PublicMemoryState {
    next_address: usize,
    regions: Vec<PublicMemoryRegion>,
    payload: HashMap<usize, RuntimeValue>,
}

impl Default for PublicMemoryState {
    fn default() -> Self {
        Self {
            next_address: PUBLIC_MEMORY_BASE,
            regions: Vec::new(),
            payload: HashMap::new(),
        }
    }
}

fn runtime_type_width(ty: TypeIR) -> usize {
    match ty {
        TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => 1,
        TypeIR::U16 | TypeIR::I16 => 2,
        TypeIR::U32 | TypeIR::I32 => 4,
        TypeIR::FixedArray { element, size } => {
            let element = match element {
                crate::ir::ScalarTypeIR::U8
                | crate::ir::ScalarTypeIR::I8
                | crate::ir::ScalarTypeIR::Logica => 1,
                crate::ir::ScalarTypeIR::U16 | crate::ir::ScalarTypeIR::I16 => 2,
                crate::ir::ScalarTypeIR::U32 | crate::ir::ScalarTypeIR::I32 => 4,
                _ => 8,
            };
            element * usize::try_from(size).unwrap_or(usize::MAX)
        }
        _ => 8,
    }
}

fn public_memory_value_to_word(value: RuntimeValue, ty: TypeIR) -> Result<u64, PinkerError> {
    match (value, ty) {
        (RuntimeValue::Int(value), ty) if ty.is_integer() => Ok(value),
        (RuntimeValue::IntSigned(value), ty) if ty.is_integer() => Ok(value as u64),
        (RuntimeValue::Bool(value), TypeIR::Logica) => Ok(u64::from(value)),
        (
            RuntimeValue::Ptr(value),
            TypeIR::Pointer { .. } | TypeIR::FunctionPointer | TypeIR::TraitObject,
        ) => Ok(value as u64),
        (RuntimeValue::Callable(value), TypeIR::Function) => Ok(value),
        (_, ty) => Err(runtime_err(&format!(
            "valor incompatível com escrita de memória pública do tipo '{ty:?}'"
        ))),
    }
}

fn public_memory_word_to_value(word: u64, ty: TypeIR) -> Result<RuntimeValue, PinkerError> {
    let value = match ty {
        TypeIR::Bombom | TypeIR::U64 => RuntimeValue::Int(word),
        TypeIR::U8 => RuntimeValue::Int(word & u8::MAX as u64),
        TypeIR::U16 => RuntimeValue::Int(word & u16::MAX as u64),
        TypeIR::U32 => RuntimeValue::Int(word & u32::MAX as u64),
        TypeIR::I8 => RuntimeValue::IntSigned((word as u8 as i8) as i64),
        TypeIR::I16 => RuntimeValue::IntSigned((word as u16 as i16) as i64),
        TypeIR::I32 => RuntimeValue::IntSigned((word as u32 as i32) as i64),
        TypeIR::I64 => RuntimeValue::IntSigned(word as i64),
        TypeIR::Logica => RuntimeValue::Bool((word & u8::MAX as u64) != 0),
        TypeIR::Pointer { .. } | TypeIR::FunctionPointer | TypeIR::TraitObject => {
            RuntimeValue::Ptr(word as usize)
        }
        TypeIR::Function => RuntimeValue::Callable(word),
        _ => {
            return Err(runtime_err(&format!(
                "tipo '{ty:?}' não possui representação escalar em memória pública"
            )));
        }
    };
    Ok(value)
}

fn public_memory_store_bytes(
    memory: &mut HashMap<usize, RuntimeValue>,
    address: usize,
    ty: TypeIR,
    value: RuntimeValue,
) -> Result<(), PinkerError> {
    let width = runtime_type_width(ty);
    let word = public_memory_value_to_word(value, ty)?;
    for offset in 0..width {
        let byte_address = address
            .checked_add(offset)
            .ok_or_else(|| runtime_err("overflow ao escrever memória pública"))?;
        memory.insert(
            byte_address,
            RuntimeValue::Int((word >> (offset * 8)) & u8::MAX as u64),
        );
    }
    Ok(())
}

fn public_memory_load_bytes(
    memory: &HashMap<usize, RuntimeValue>,
    address: usize,
    ty: TypeIR,
) -> Result<RuntimeValue, PinkerError> {
    let width = runtime_type_width(ty);
    let mut word = 0u64;
    for offset in 0..width {
        let byte_address = address
            .checked_add(offset)
            .ok_or_else(|| runtime_err("overflow ao ler memória pública"))?;
        let byte = match memory.get(&byte_address) {
            Some(RuntimeValue::Int(value)) => *value & u8::MAX as u64,
            None => 0,
            Some(_) => {
                return Err(runtime_err(
                    "representação interna inválida em byte de memória pública",
                ));
            }
        };
        word |= byte << (offset * 8);
    }
    public_memory_word_to_value(word, ty)
}

fn public_memory_interval_contained(
    region_start: usize,
    region_size: usize,
    access_start: usize,
    access_width: usize,
) -> Result<bool, PinkerError> {
    let region_end = region_start.checked_add(region_size).ok_or_else(|| {
        runtime_err("E-RUNTIME-MEM-ADDRESS-OVERFLOW: metadados de região pública inválidos")
    })?;
    let access_end = access_start.checked_add(access_width).ok_or_else(|| {
        runtime_err("E-RUNTIME-MEM-ADDRESS-OVERFLOW: overflow no acesso à memória pública")
    })?;
    Ok(access_start >= region_start && access_end <= region_end)
}

fn public_memory_region(state: &PublicMemoryState, address: usize) -> Option<(usize, usize, bool)> {
    for region in state.regions.iter().rev() {
        if region
            .base
            .checked_add(region.size)
            .is_some_and(|end| address >= region.base && address < end)
        {
            return Some((region.base, region.size, region.alive));
        }
    }
    for region in state.regions.iter().rev() {
        if region
            .base
            .checked_add(region.size)
            .is_some_and(|end| address == end)
        {
            return Some((region.base, region.size, region.alive));
        }
    }
    None
}

fn public_memory_access_region(
    state: &PublicMemoryState,
    address: usize,
    width: usize,
) -> Result<Option<(usize, usize, bool)>, PinkerError> {
    let access_end = address.checked_add(width).ok_or_else(|| {
        runtime_err("E-RUNTIME-MEM-ADDRESS-OVERFLOW: overflow no acesso à memória pública")
    })?;
    Ok(state.regions.iter().rev().find_map(|region| {
        let region_end = region.base.checked_add(region.size)?;
        ((address >= region.base && address <= region_end)
            || (address < region.base && access_end > region.base))
            .then_some((region.base, region.size, region.alive))
    }))
}

fn validar_derivacao_memoria_publica(
    state: &PublicMemoryState,
    origem: Option<usize>,
    resultado: &RuntimeValue,
) -> Result<(), PinkerError> {
    let (Some(origem), RuntimeValue::Ptr(derivado)) = (origem, resultado) else {
        return Ok(());
    };
    let Some((base, size, alive)) = public_memory_region(state, origem) else {
        return Ok(());
    };
    if !alive {
        return Err(runtime_err(
            "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em memória pública",
        ));
    }
    let fim = base.checked_add(size).ok_or_else(|| {
        runtime_err("E-RUNTIME-MEM-ADDRESS-OVERFLOW: metadados de região pública inválidos")
    })?;
    if *derivado < base || *derivado > fim {
        return Err(runtime_err(
            "E-RUNTIME-MEM-OUT-OF-BOUNDS: derivação fora dos limites da alocação pública",
        ));
    }
    Ok(())
}

fn public_memory_allocate(
    args: &[RuntimeValue],
    state: &mut PublicMemoryState,
) -> Result<IntrinsicCall, PinkerError> {
    debug_assert_eq!(
        PUBLIC_MEMORY_MAX_METADATA_BYTES,
        PUBLIC_MEMORY_MAX_IDENTITIES * std::mem::size_of::<PublicMemoryRegion>()
    );
    debug_assert_eq!(PUBLIC_MEMORY_MAX_QUARANTINE_BYTES, 0);
    let [RuntimeValue::Int(size)] = args else {
        return Err(runtime_err("'alocar' exige um tamanho 'u64' em bytes"));
    };
    let size = usize::try_from(*size)
        .map_err(|_| runtime_err("'alocar' recebeu tamanho que excede a plataforma"))?;
    if size == 0 {
        return Err(runtime_err("'alocar' rejeita tamanho zero"));
    }
    if size > (isize::MAX as usize).saturating_sub(16) {
        return Err(runtime_err(
            "'alocar' excede o maior bloco representável pela plataforma",
        ));
    }
    let rounded = size
        .checked_add(15)
        .map(|value| value & !15)
        .ok_or_else(|| runtime_err("overflow ao alinhar tamanho de 'alocar'"))?;
    let base = state.next_address;
    let next = base
        .checked_add(rounded)
        .ok_or_else(|| runtime_err("overflow de endereço em 'alocar'"))?;
    let arena_end = PUBLIC_MEMORY_BASE
        .checked_add(PUBLIC_MEMORY_MAX_VIRTUAL_BYTES)
        .ok_or_else(|| runtime_err("overflow no limite virtual da memória pública"))?;
    if state.regions.len() >= PUBLIC_MEMORY_MAX_IDENTITIES {
        return Err(runtime_err("limite de identidades públicas esgotado"));
    }
    if next > arena_end {
        return Err(runtime_err("espaço virtual público esgotado"));
    }
    state
        .regions
        .try_reserve(1)
        .map_err(|_| runtime_err("registro de alocações públicas não pôde reservar metadata"))?;
    state.next_address = next;
    state.regions.push(PublicMemoryRegion {
        base,
        size,
        alive: true,
    });
    Ok(IntrinsicCall::Done(Some(RuntimeValue::Ptr(base))))
}

fn public_memory_free(
    args: &[RuntimeValue],
    state: &mut PublicMemoryState,
) -> Result<IntrinsicCall, PinkerError> {
    let [RuntimeValue::Ptr(pointer)] = args else {
        return Err(runtime_err("'liberar' exige um ponteiro-base 'seta<u8>'"));
    };
    if *pointer == 0 {
        return Err(runtime_err("'liberar' rejeita ponteiro nulo"));
    }
    for region in state.regions.iter_mut().rev() {
        if region.base != *pointer {
            continue;
        }
        if !region.alive {
            return Err(runtime_err(
                "E-RUNTIME-MEM-DOUBLE-FREE: 'liberar' detectou double free",
            ));
        }
        region.alive = false;
        let base = region.base;
        let end = base
            .checked_add(region.size)
            .ok_or_else(|| runtime_err("overflow em metadata de memória pública"))?;
        state
            .payload
            .retain(|address, _| *address < base || *address >= end);
        return Ok(IntrinsicCall::Done(None));
    }
    if state.regions.iter().any(|region| {
        region
            .base
            .checked_add(region.size)
            .is_some_and(|end| *pointer > region.base && *pointer < end)
    }) {
        return Err(runtime_err(
            "E-RUNTIME-MEM-INTERIOR-FREE: 'liberar' rejeita ponteiro interior; use o ponteiro-base",
        ));
    }
    Err(runtime_err(
        "E-RUNTIME-MEM-FOREIGN-FREE: 'liberar' rejeita ponteiro estrangeiro ou de domínio interno",
    ))
}

fn try_call_intrinsic(
    callee: &str,
    args: &[RuntimeValue],
    public_memory_state: &mut PublicMemoryState,
    io_state: &mut RuntimeIoState,
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
) -> Result<IntrinsicCall, PinkerError> {
    match callee {
        "alocar" => public_memory_allocate(args, public_memory_state),
        "liberar" => public_memory_free(args, public_memory_state),
        "aleatorio_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige 1 argumento (semente bombom)",
                ));
            }
            let RuntimeValue::Int(seed) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige semente bombom",
                ));
            };
            let handle = random_state.next_generator_handle;
            random_state.next_generator_handle =
                random_state.next_generator_handle.saturating_add(1);
            random_state
                .generators
                .insert(handle, RuntimeRandomGenerator { state: seed });
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "aleatorio_proximo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige 1 argumento (gerador bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige gerador bombom",
                ));
            };
            let Some(generator) = random_state.generators.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de aleatoriedade inválido em 'aleatorio_proximo'",
                ));
            };
            let next = advance_random_generator(&mut generator.state);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(next))))
        }
        // @pinker-nav:end interpreter.intrinsecos.acaso

        // @pinker-nav:start interpreter.intrinsecos.listas
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa operações hospedadas contíguas de listas de bombom e verso, criando handles tipados, anexando, obtendo, medindo, definindo, removendo e inserindo elementos com validação dinâmica de aridade, índice, handle e tipo; os handles pertencem ao estado do interpretador.
        "lista_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = list_state.next_list_handle;
            list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
            list_state.lists_bombom.insert(handle, Vec::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "lista_bombom_anexar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            lista.push(value);
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.get(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_obter'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "lista_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                lista.len() as u64
            ))))
        }
        "lista_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige 3 argumentos (lista<bombom>, bombom, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(slot) = lista.get_mut(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_definir'",
                ));
            };
            *slot = value;
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_tirar_ultimo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.pop() else {
                return Err(runtime_err("lista vazia em 'lista_bombom_tirar_ultimo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(value))))
        }
        "lista_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = list_state.next_list_handle;
            list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
            list_state.lists_verso.insert(handle, Vec::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListVerso(handle))))
        }
        "lista_verso_anexar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige 2 argumentos (lista<verso>, verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(value) = args[1].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige verso no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            lista.push(value);
            Ok(IntrinsicCall::Done(None))
        }
        "lista_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige 2 argumentos (lista<verso>, bombom)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(value) = lista.get(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_verso_obter'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "lista_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tamanho' exige 1 argumento (lista<verso>)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tamanho' exige lista<verso> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                lista.len() as u64
            ))))
        }
        "lista_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige 3 argumentos (lista<verso>, bombom, verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Str(value) = args[2].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(slot) = lista.get_mut(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_verso_definir'",
                ));
            };
            *slot = value;
            Ok(IntrinsicCall::Done(None))
        }
        "lista_verso_tirar_ultimo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tirar_ultimo' exige 1 argumento (lista<verso>)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tirar_ultimo' exige lista<verso> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(value) = lista.pop() else {
                return Err(runtime_err("lista vazia em 'lista_verso_tirar_ultimo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        "lista_verso_inserir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige 3 argumentos (lista, índice bombom, valor verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige lista<verso>",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige índice bombom",
                ));
            };
            let RuntimeValue::Str(valor) = args[2].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige valor verso",
                ));
            };
            let lista = list_state
                .lists_verso
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'lista_verso_inserir': lista inválida"))?;
            let idx = index as usize;
            if idx > lista.len() {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir': índice fora dos limites",
                ));
            }
            lista.insert(idx, valor);
            Ok(IntrinsicCall::Done(None))
        }
        // @pinker-nav:end interpreter.intrinsecos.listas

        // @pinker-nav:start interpreter.intrinsecos.mapas-verso-bombom
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa o primeiro bloco contíguo de mapa hospedado `verso -> bombom`, incluindo criação, escrita, leitura, presença, tamanho e cursores internos usados por lowering de iteração; valida aridade, handles e chaves sem definir layout nativo.
        "mapa_verso_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_verso_bombom.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "mapa_verso_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige 3 argumentos (mapa<verso,bombom>, verso, bombom)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige verso no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_definir'",
                ));
            };
            mapa.insert(key.clone(), value);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_obter'",
                ));
            };
            let Some(value) = mapa.get(key) else {
                return Err(runtime_err("chave ausente em 'mapa_verso_bombom_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "mapa_verso_bombom_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(key),
            ))))
        }
        "mapa_verso_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em '__pinker_internal_mapa_verso_bombom_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_verso_bombom.insert(
                iter_handle,
                RuntimeMapVersoBombomIter {
                    keys_snapshot: mapa.keys().cloned().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_verso_bombom.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                )
            })?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(key.clone()))))
        }
        // @pinker-nav:end interpreter.intrinsecos.mapas-verso-bombom

        // @pinker-nav:start interpreter.intrinsecos.leques
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa leques hospedados por handle opaco, criando valores, anexando payload inteiro ou textual e carregando tag ou carga com validações de handle, tag e índice; descreve somente a representação do interpretador, não o layout futuro do runtime nativo.
        "__pinker_internal_leque_criar_0" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_criar_0' exige 1 argumento (tag)",
                ));
            }
            let RuntimeValue::Int(tag) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_criar_0' exige tag 'bombom'",
                ));
            };
            let handle = map_state.next_enum_handle;
            map_state.next_enum_handle = map_state.next_enum_handle.saturating_add(1);
            map_state.enum_values.insert(handle, (*tag, Vec::new()));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "__pinker_internal_leque_anexar_b" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_b' exige 2 argumentos (leque, carga)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(payload)) = (&args[0], &args[1])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_b' exige handle e carga 'bombom'",
                ));
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_anexar_b'",
                ));
            };
            payloads.push(RuntimeEnumPayload::Int(*payload));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        "__pinker_internal_leque_anexar_v" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_v' exige 2 argumentos (leque, carga)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Str(payload)) = (&args[0], &args[1])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_v' exige handle 'bombom' e carga 'verso'",
                ));
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_anexar_v'",
                ));
            };
            payloads.push(RuntimeEnumPayload::Str(payload.clone()));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        "__pinker_internal_uniao_tag" => {
            let [RuntimeValue::Ptr(handle)] = args else {
                return Err(runtime_err("tag de união exige handle de união"));
            };
            let tag = UNION_RUNTIME_STATE.with(|state| {
                state
                    .borrow()
                    .descriptors
                    .get(handle)
                    .map(|descriptor| descriptor.tag)
                    .ok_or_else(|| runtime_err("handle de união inválido"))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(tag))))
        }
        "__pinker_internal_uniao_payload_b" | "__pinker_internal_uniao_payload_v" => {
            let [RuntimeValue::Ptr(handle), RuntimeValue::Int(expected_tag)] = args else {
                return Err(runtime_err("payload de união exige handle e tag"));
            };
            let descriptor = UNION_RUNTIME_STATE.with(|state| {
                state
                    .borrow()
                    .descriptors
                    .get(handle)
                    .cloned()
                    .ok_or_else(|| runtime_err("handle de união inválido"))
            })?;
            if descriptor.tag != *expected_tag {
                return Err(runtime_err("tag divergente ao abrir payload de união"));
            }
            if descriptor.payload_size == 0
                || descriptor.payload_align == 0
                || !descriptor.payload_align.is_power_of_two()
            {
                return Err(runtime_err("layout inválido no descritor de união"));
            }
            if callee == "__pinker_internal_uniao_payload_v"
                && !matches!(descriptor.payload, RuntimeValue::Str(_))
            {
                return Err(runtime_err("payload de união não é verso"));
            }
            Ok(IntrinsicCall::Done(Some(descriptor.payload)))
        }
        "__pinker_internal_leque_tag" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_tag' exige 1 argumento (leque)",
                ));
            }
            let RuntimeValue::Int(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_tag' exige handle 'bombom'",
                ));
            };
            let Some((tag, _)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_tag'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*tag))))
        }
        "__pinker_internal_leque_carga_b" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_b' exige 3 argumentos (leque, tag, índice)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_b' exige argumentos 'bombom'",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_carga_b'",
                ));
            };
            if stored_tag != tag {
                return Err(runtime_err(
                    "extração de carga com variante inconsistente em '__pinker_internal_leque_carga_b'",
                ));
            }
            let Some(RuntimeEnumPayload::Int(value)) = payloads.get(*index as usize) else {
                return Err(runtime_err(
                    "carga 'bombom' ausente em '__pinker_internal_leque_carga_b'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "__pinker_internal_leque_carga_v" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_v' exige 3 argumentos (leque, tag, índice)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_v' exige argumentos 'bombom'",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_carga_v'",
                ));
            };
            if stored_tag != tag {
                return Err(runtime_err(
                    "extração de carga com variante inconsistente em '__pinker_internal_leque_carga_v'",
                ));
            }
            let Some(RuntimeEnumPayload::Str(value)) = payloads.get(*index as usize) else {
                return Err(runtime_err(
                    "carga 'verso' ausente em '__pinker_internal_leque_carga_v'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        // @pinker-nav:end interpreter.intrinsecos.leques

        // @pinker-nav:start interpreter.intrinsecos.io-arquivo-texto
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Agrupa intrínsecas hospedadas contíguas de stdin, arquivos por handle, operações diretas de texto e serialização mínima, validando aridade e tipos, lendo stdin, escrevendo stdout ou filesystem real e retornando valores Pinker; não é filesystem virtual nem runtime nativo.
        "ouvir" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir' exige 0 argumentos"));
            }
            let mut raw = String::new();
            io::stdin()
                .read_line(&mut raw)
                .map_err(|err| runtime_err(&format!("falha ao ler stdin em 'ouvir': {}", err)))?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "entrada inválida para 'ouvir': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "entrada inválida para 'ouvir': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "ouvir_verso" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir_verso' exige 0 argumentos"));
            }
            let maybe_line = read_stdin_line_minima("ouvir_verso")?;
            let Some(line) = maybe_line else {
                return Err(runtime_err(
                    "falha ao ler stdin em 'ouvir_verso': EOF imediato sem linha disponível",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                trim_final_newline_minimo(line),
            ))))
        }
        "ouvir_verso_ou" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(default_value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige valor padrão em verso",
                ));
            };
            match read_stdin_line_minima("ouvir_verso_ou") {
                Ok(Some(line)) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    trim_final_newline_minimo(line),
                )))),
                Ok(None) | Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
        }
        "abrir" => {
            if args.len() != 1 {
                return Err(runtime_err("intrínseca 'abrir' exige 1 argumento (verso)"));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'abrir' exige caminho em verso"));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!("falha ao abrir arquivo em 'abrir': {}", err))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                    append_enabled: false,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "criar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige caminho em verso",
                ));
            };
            fs::write(path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar arquivo em 'criar_arquivo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content: String::new(),
                    append_enabled: false,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "abrir_anexo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige caminho em verso",
                ));
            };
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .map_err(|err| {
                    runtime_err(&format!("falha ao abrir arquivo em 'abrir_anexo': {}", err))
                })?;
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao carregar conteúdo em 'abrir_anexo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                    append_enabled: true,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "ler_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'ler_arquivo' exige handle bombom"));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_arquivo'"));
            };
            let trimmed = open_file.content.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "conteúdo inválido para 'ler_arquivo': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "conteúdo inválido para 'ler_arquivo': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "ler_verso_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_verso_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_verso_arquivo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                open_file.content.clone(),
            ))))
        }
        "ler_arquivo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige caminho em verso",
                ));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao ler arquivo em 'ler_arquivo_verso': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content))))
        }
        "arquivo_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige caminho em verso",
                ));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige valor padrão em verso",
                ));
            };
            match fs::read_to_string(path) {
                Ok(content) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content)))),
                Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
        }
        "escrever" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever' exige 2 argumentos (handle, bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'escrever' exige handle bombom"));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err("intrínseca 'escrever' exige valor bombom"));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever'"));
                }
                return Err(runtime_err("handle inválido em 'escrever'"));
            };
            let next_content = value.to_string();
            fs::write(&open_file.path, &next_content).map_err(|err| {
                runtime_err(&format!("falha ao escrever arquivo em 'escrever': {}", err))
            })?;
            open_file.content = next_content;
            Ok(IntrinsicCall::Done(None))
        }
        "escrever_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige handle bombom",
                ));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever_verso'"));
                }
                return Err(runtime_err("handle inválido em 'escrever_verso'"));
            };
            fs::write(&open_file.path, value).map_err(|err| {
                runtime_err(&format!(
                    "falha ao escrever verso em arquivo em 'escrever_verso': {}",
                    err
                ))
            })?;
            open_file.content.clone_from(value);
            Ok(IntrinsicCall::Done(None))
        }
        "truncar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'truncar_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'truncar_arquivo'"));
            };
            fs::write(&open_file.path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao truncar arquivo em 'truncar_arquivo': {}",
                    err
                ))
            })?;
            open_file.content.clear();
            Ok(IntrinsicCall::Done(None))
        }
        "anexar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'anexar_verso' exige handle bombom"));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'anexar_verso'"));
                }
                return Err(runtime_err("handle inválido em 'anexar_verso'"));
            };
            if !open_file.append_enabled {
                return Err(runtime_err(
                    "handle não foi aberto com 'abrir_anexo' em 'anexar_verso'",
                ));
            }
            let mut file = OpenOptions::new()
                .append(true)
                .open(&open_file.path)
                .map_err(|err| {
                    runtime_err(&format!(
                        "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                        err
                    ))
                })?;
            use std::io::Write as _;
            file.write_all(value.as_bytes()).map_err(|err| {
                runtime_err(&format!(
                    "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                    err
                ))
            })?;
            open_file.content.push_str(value);
            Ok(IntrinsicCall::Done(None))
        }
        "fechar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'fechar' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'fechar' exige handle bombom"));
            };
            if io_state.open_files.remove(&handle).is_none() {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'fechar'"));
                }
                return Err(runtime_err("handle inválido em 'fechar'"));
            }
            io_state.closed_handles.insert(handle);
            Ok(IntrinsicCall::Done(None))
        }
        "juntar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(format!(
                "{}{}",
                lhs, rhs
            )))))
        }
        "tamanho_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                value.chars().count() as u64,
            ))))
        }
        "indice_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige 2 argumentos (verso, bombom)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige segundo argumento em bombom",
                ));
            };
            let Some(ch) = value.chars().nth(index as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'indice_verso' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(ch.to_string()))))
        }
        "contem_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.contains(trecho),
            ))))
        }
        "comeca_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(prefixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.starts_with(prefixo),
            ))))
        }
        "termina_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sufixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.ends_with(sufixo),
            ))))
        }
        "igual_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(lhs == rhs))))
        }
        "vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.is_empty(),
            ))))
        }
        "aparar_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.trim().to_string(),
            ))))
        }
        "minusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_lowercase(),
            ))))
        }
        "maiusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_uppercase(),
            ))))
        }
        "indice_verso_em" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige segundo argumento em verso",
                ));
            };
            let pos = texto.find(trecho).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        // Fase 140 — buscar_verso(texto, padrao) -> bombom
        "buscar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(padrao) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige segundo argumento em verso",
                ));
            };
            if padrao.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' não aceita padrão vazio",
                ));
            }
            let pos = texto.find(padrao).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        "nao_vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                !texto.is_empty(),
            ))))
        }
        // Fase 137 — dividir_verso_em(texto, sep, indice) -> verso
        "dividir_verso_em" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige 3 argumentos (verso, verso, bombom)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Int(indice) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige terceiro argumento em bombom",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' não aceita separador vazio",
                ));
            }
            let partes: Vec<&str> = texto.split(sep.as_str()).collect();
            let Some(parte) = partes.get(indice as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'dividir_verso_em' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                parte.to_string(),
            ))))
        }
        // Fase 137 — dividir_verso_contar(texto, sep) -> bombom
        "dividir_verso_contar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige segundo argumento em verso",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' não aceita separador vazio",
                ));
            }
            let count = texto.split(sep.as_str()).count() as u64;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(count))))
        }
        // Fase 138 — substituir_verso(texto, de, para) -> verso
        "substituir_verso" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(de) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(para) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige terceiro argumento em verso",
                ));
            };
            if de.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' não aceita padrão vazio",
                ));
            }
            let resultado = texto.replace(de.as_str(), para.as_str());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        // Fase 139 — juntar_verso_com(a, sep, b) -> verso
        "juntar_verso_com" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(a) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(b) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige terceiro argumento em verso",
                ));
            };
            let resultado = format!("{}{}{}", a, sep, b);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "formatar_verso" => {
            if args.len() < 2 {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige pelo menos 2 argumentos (modelo verso, args...)",
                ));
            }
            let RuntimeValue::Str(modelo) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige modelo em verso",
                ));
            };
            let resultado = formatar_verso_runtime(modelo, &args[1..])?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "__ternario" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca '__ternario' exige 3 argumentos (condição, valor_verdade, valor_falso)",
                ));
            }
            let cond = match &args[0] {
                RuntimeValue::Bool(b) => *b,
                _ => {
                    return Err(runtime_err("intrínseca '__ternario' exige condição logica"));
                }
            };
            let result = if cond {
                args[1].clone()
            } else {
                args[2].clone()
            };
            Ok(IntrinsicCall::Done(Some(result)))
        }
        "ler_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige 2 argumentos (linha verso, separador verso)",
                ));
            }
            let RuntimeValue::Str(linha) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige linha em verso",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige separador em verso",
                ));
            };
            let separador = validar_separador_csv("ler_linha_csv_bombom", separador)?;
            if linha.contains('\n') || linha.contains('\r') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': multiline fora do recorte",
                ));
            }
            if linha.contains('"') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': quoting fora do recorte",
                ));
            }

            let handle = list_state.next_list_handle;
            list_state.next_list_handle += 1;
            let mut itens = Vec::new();
            for campo in linha.split(separador) {
                let Ok(valor) = campo.parse::<u64>() else {
                    return Err(runtime_err(
                        "campo inválido em 'ler_linha_csv_bombom': esperado bombom simples sem quoting",
                    ));
                };
                itens.push(valor);
            }
            list_state.lists_bombom.insert(handle, itens);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "emitir_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige 2 argumentos (lista<bombom>, separador verso)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige separador em verso no segundo argumento",
                ));
            };
            let separador = validar_separador_csv("emitir_linha_csv_bombom", separador)?;
            let Some(itens) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de lista inválido em 'emitir_linha_csv_bombom'",
                ));
            };
            let linha = itens
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(separador);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(linha))))
        }
        "ler_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige 1 argumento (json verso)",
                ));
            }
            let RuntimeValue::Str(json) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige json em verso",
                ));
            };
            let handle = map_state.next_map_handle;
            map_state.next_map_handle += 1;
            let mapa = parse_json_plano_bombom(json)?;
            map_state.maps_verso_bombom.insert(handle, mapa);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "emitir_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige mapa<verso,bombom> no argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'emitir_json_plano_bombom'",
                ));
            };
            let json = emit_json_plano_bombom(mapa)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(json))))
        }
        // @pinker-nav:end interpreter.intrinsecos.io-arquivo-texto

        // @pinker-nav:start interpreter.intrinsecos.tempo-processos-ambiente
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Agrupa intrínsecas hospedadas contíguas de relógio, processos, argumentos CLI, ambiente, caminhos, status de saída, assertivas, espera e cópia ou renomeação, com efeitos reais no host como `Command`, pipes, diretório atual, variáveis de ambiente, sono e filesystem; devolve resultados ao interpretador sem prometer modo freestanding.
        "tempo_unix" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'tempo_unix' exige 0 argumentos"));
            }
            let agora = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| {
                    runtime_err("intrínseca 'tempo_unix' não suporta tempo anterior à época Unix")
                })?
                .as_secs();
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(agora))))
        }
        "formatar_tempo_unix" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'formatar_tempo_unix' exige 1 argumento (timestamp bombom)",
                ));
            }
            let RuntimeValue::Int(timestamp) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'formatar_tempo_unix' exige timestamp em bombom",
                ));
            };
            let texto = formatar_tempo_unix_iso_utc(timestamp)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(texto))))
        }
        "executar_processo" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'executar_processo' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'executar_processo' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'executar_processo' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let exit_code = executar_processo_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "executar_com_entrada" => {
            if !(2..=3).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige 2 ou 3 argumentos (comando verso, entrada verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige comando em verso",
                ));
            };
            let RuntimeValue::Str(input_text) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige entrada em verso",
                ));
            };
            let explicit_argv = match args.get(2) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'executar_com_entrada' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let exit_code = executar_com_entrada_minimo(command_name, input_text, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "pipeline_minimo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige 2 argumentos (produtor verso, consumidor verso)",
                ));
            }
            let RuntimeValue::Str(producer_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige produtor em verso",
                ));
            };
            let RuntimeValue::Str(consumer_name) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige consumidor em verso",
                ));
            };
            let exit_code = pipeline_minimo(producer_name, consumer_name)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "capturar_stdout" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'capturar_stdout' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'capturar_stdout' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'capturar_stdout' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let stdout = capturar_stdout_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(stdout))))
        }
        "capturar_stderr" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'capturar_stderr' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'capturar_stderr' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'capturar_stderr' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let stderr = capturar_stderr_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(stderr))))
        }
        "argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento' exige índice bombom"));
            };
            let Some(arg) = io_state.cli_args.get(index as usize) else {
                return Err(runtime_err("índice fora da faixa em 'argumento'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(arg.clone()))))
        }
        "argumento_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige 2 argumentos (índice bombom, padrão verso)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento_ou' exige índice bombom"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige valor padrão em verso",
                ));
            };
            let value = io_state
                .cli_args
                .get(index as usize)
                .cloned()
                .unwrap_or_else(|| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        intrinsic_name @ ("tem_chave" | "tem_argumento_nomeado") => {
            if args.len() != 1 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 1 argumento (chave verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            let found = matches!(
                find_named_cli_argument(&io_state.cli_args, key),
                NamedArgLookup::PresentValue(_)
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
        }
        intrinsic_name @ ("pedir_argumento" | "argumento_nomeado_ou") => {
            if args.len() != 2 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 2 argumentos (chave verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            match find_named_cli_argument(&io_state.cli_args, key) {
                NamedArgLookup::Missing => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
                NamedArgLookup::PresentValue(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                NamedArgLookup::PresentWithoutValue => Err(runtime_err(&format!(
                    "intrínseca '{}' encontrou chave '{}' sem valor na forma '--chave valor'",
                    intrinsic_name, key
                ))),
            }
        }
        "tem_flag" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_flag' exige 1 argumento (chave verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'tem_flag' exige chave em verso"));
            };
            ensure_named_arg_key_valid("tem_flag", key)?;
            let found = io_state.cli_args.iter().any(|a| a == key);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
        }
        "ambiente_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige 2 argumentos (chave verso, padrão verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'ambiente_ou' exige chave em verso"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige valor padrão em verso",
                ));
            };
            let value = env::var(key).unwrap_or_else(|_| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        intrinsic_name @ ("buscar_contexto" | "argumento_nomeado_ou_ambiente_ou") => {
            if args.len() != 3 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 3 argumentos (chave_arg verso, chave_env verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(arg_key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_arg em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(env_key) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_env em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[2] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, arg_key)?;
            ensure_env_key_valid(intrinsic_name, env_key)?;
            match find_named_cli_argument(&io_state.cli_args, arg_key) {
                NamedArgLookup::PresentValue(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                NamedArgLookup::PresentWithoutValue => Err(runtime_err(&format!(
                    "intrínseca '{}' encontrou chave '{}' sem valor na forma '--chave valor'",
                    intrinsic_name, arg_key
                ))),
                NamedArgLookup::Missing => {
                    let value = env::var(env_key).unwrap_or_else(|_| default_value.clone());
                    Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
                }
            }
        }
        "caminho_existe" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).exists(),
            ))))
        }
        "e_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_arquivo' exige caminho em verso"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_file(),
            ))))
        }
        "e_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_dir(),
            ))))
        }
        "juntar_caminho" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige 2 argumentos (base verso, trecho verso)",
                ));
            }
            let RuntimeValue::Str(base) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige base em verso",
                ));
            };
            let RuntimeValue::Str(child) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige trecho em verso",
                ));
            };
            let joined = std::path::PathBuf::from(base).join(child);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                joined.to_string_lossy().to_string(),
            ))))
        }
        "tamanho_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho em verso",
                ));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter metadados em 'tamanho_arquivo': {}",
                    err
                ))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(metadata.len()))))
        }
        "e_vazio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_vazio' exige caminho em verso"));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!("falha ao obter metadados em 'e_vazio': {}", err))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                metadata.len() == 0,
            ))))
        }
        "criar_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige caminho em verso",
                ));
            };
            fs::create_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar diretório em 'criar_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige caminho em verso",
                ));
            };
            fs::remove_file(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover arquivo em 'remover_arquivo': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige caminho em verso",
                ));
            };
            fs::remove_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover diretório em 'remover_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "diretorio_atual" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'diretorio_atual' exige 0 argumentos",
                ));
            }
            let value = env::current_dir().map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter diretório atual em 'diretorio_atual': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                value.to_string_lossy().to_string(),
            ))))
        }
        "quantos_argumentos" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'quantos_argumentos' exige 0 argumentos",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                io_state.cli_args.len() as u64,
            ))))
        }
        "tem_argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige índice bombom",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                io_state.cli_args.get(index as usize).is_some(),
            ))))
        }
        "sair" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'sair' exige 1 argumento (código bombom)",
                ));
            }
            let RuntimeValue::Int(code) = args[0] else {
                return Err(runtime_err("intrínseca 'sair' exige código bombom"));
            };
            io_state.exit_status = Some(code.min(i32::MAX as u64) as i32);
            Ok(IntrinsicCall::Done(None))
        }
        "afirmar" => {
            if args.is_empty() || args.len() > 2 {
                return Err(runtime_err(
                    "intrínseca 'afirmar' exige 1 ou 2 argumentos (condição logica [, mensagem verso])",
                ));
            }
            let RuntimeValue::Bool(cond) = args[0] else {
                return Err(runtime_err("intrínseca 'afirmar' exige condição logica"));
            };
            if !cond {
                let msg = if args.len() == 2 {
                    if let RuntimeValue::Str(ref s) = args[1] {
                        format!("afirmação falhou: {}", s)
                    } else {
                        "afirmação falhou".to_string()
                    }
                } else {
                    "afirmação falhou".to_string()
                };
                return Err(runtime_err(&msg));
            }
            Ok(IntrinsicCall::Done(None))
        }
        "dormir" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'dormir' exige 1 argumento (milissegundos bombom)",
                ));
            }
            let RuntimeValue::Int(ms) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dormir' exige milissegundos bombom",
                ));
            };
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Ok(IntrinsicCall::Done(None))
        }
        "copiar_arquivo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige 2 argumentos (origem verso, destino verso)",
                ));
            }
            let RuntimeValue::Str(ref origem) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige origem verso",
                ));
            };
            let RuntimeValue::Str(ref destino) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige destino verso",
                ));
            };
            fs::copy(origem, destino).map_err(|err| {
                runtime_err(&format!(
                    "falha ao copiar '{}' para '{}': {}",
                    origem, destino, err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "renomear_arquivo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 2 argumentos (de verso, para verso)",
                ));
            }
            let RuntimeValue::Str(ref de) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 'de' verso",
                ));
            };
            let RuntimeValue::Str(ref para) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 'para' verso",
                ));
            };
            fs::rename(de, para).map_err(|err| {
                runtime_err(&format!(
                    "falha ao renomear '{}' para '{}': {}",
                    de, para, err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        // @pinker-nav:end interpreter.intrinsecos.tempo-processos-ambiente

        // @pinker-nav:start interpreter.intrinsecos.conversoes-numero-texto
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Intrínsecas hospedadas de conversão entre número e texto: `verso_para_bombom` (parse de `verso` para `bombom`, com erro em texto inválido) e `bombom_para_verso` (formatação de `bombom` como `verso`). Valida aridade e tipos dos argumentos.
        "verso_para_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'verso_para_bombom' exige 1 argumento (texto verso)",
                ));
            }
            let RuntimeValue::Str(ref texto) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'verso_para_bombom' exige texto verso",
                ));
            };
            let parsed: u64 = texto
                .trim()
                .parse()
                .map_err(|_| runtime_err(&format!("falha ao converter '{}' para bombom", texto)))?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "bombom_para_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'bombom_para_verso' exige 1 argumento (valor bombom)",
                ));
            }
            let RuntimeValue::Int(valor) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'bombom_para_verso' exige valor bombom",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                valor.to_string(),
            ))))
        }
        // @pinker-nav:end interpreter.intrinsecos.conversoes-numero-texto

        // Arm isolado da família `acaso` (ver `interpreter.intrinsecos.acaso`),
        // fisicamente separado dela neste ponto do dispatcher; sem âncora própria.
        "aleatorio_entre" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre' exige 3 argumentos (gerador bombom, min bombom, max bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre' exige gerador bombom",
                ));
            };
            let RuntimeValue::Int(min) = args[1] else {
                return Err(runtime_err("intrínseca 'aleatorio_entre' exige min bombom"));
            };
            let RuntimeValue::Int(max) = args[2] else {
                return Err(runtime_err("intrínseca 'aleatorio_entre' exige max bombom"));
            };
            if min > max {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre': min não pode ser maior que max",
                ));
            }
            let generator = random_state
                .generators
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'aleatorio_entre': gerador inválido"))?;
            let raw = advance_random_generator(&mut generator.state);
            let range = max - min + 1;
            let result = if range == 0 { raw } else { min + (raw % range) };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(result))))
        }

        // @pinker-nav:start interpreter.intrinsecos.mapas-tipados
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Intrínsecas hospedadas das famílias tipadas de mapa `mapa<verso,verso>`, `mapa<bombom,bombom>` e `mapa<bombom,verso>` — cada uma com `criar`/`definir`/`obter`/`tem`/`tamanho`/`remover` e os cursores internos de iteração (`__pinker_internal_..._iterador_criar`/`_proxima_chave`) — mais a remoção residual de `mapa<verso,bombom>`. Opera sobre as tabelas de estado do hospedeiro; valida aridade e tipos.
        "mapa_verso_bombom_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige 2 argumentos (mapa, chave verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige mapa<verso,bombom>",
                ));
            };
            let RuntimeValue::Str(ref chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige chave verso",
                ));
            };
            let mapa = map_state
                .maps_verso_bombom
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_verso_bombom_remover': mapa inválido")
                })?;
            mapa.remove(chave);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_verso_verso.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoVerso(
                handle,
            ))))
        }
        "mapa_verso_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige 3 argumentos (mapa<verso,verso>, verso, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige verso no segundo argumento",
                ));
            };
            let RuntimeValue::Str(ref value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_definir'",
                ));
            };
            mapa.insert(key.clone(), value.clone());
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige 2 argumentos (mapa<verso,verso>, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_obter'",
                ));
            };
            let Some(value) = mapa.get(key) else {
                return Err(runtime_err("chave ausente em 'mapa_verso_verso_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "mapa_verso_verso_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige 2 argumentos (mapa<verso,verso>, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(key),
            ))))
        }
        "mapa_verso_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tamanho' exige 1 argumento (mapa<verso,verso>)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tamanho' exige mapa<verso,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_verso_verso_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige 2 argumentos (mapa, chave verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige mapa<verso,verso>",
                ));
            };
            let RuntimeValue::Str(ref chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige chave verso",
                ));
            };
            let mapa = map_state.maps_verso_verso.get_mut(&handle).ok_or_else(|| {
                runtime_err("intrínseca 'mapa_verso_verso_remover': mapa inválido")
            })?;
            mapa.remove(chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_verso_verso_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_criar' exige 1 argumento (mapa<verso,verso>)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_criar' exige mapa<verso,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em '__pinker_internal_mapa_verso_verso_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_verso_verso.insert(
                iter_handle,
                RuntimeMapVersoVersoIter {
                    keys_snapshot: mapa.keys().cloned().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_verso_verso_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_verso_verso.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_verso_verso_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_verso_verso_iterador_proxima_chave'",
                )
            })?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(key.clone()))))
        }
        "mapa_bombom_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_bombom_bombom.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapBombomBombom(
                handle,
            ))))
        }
        "mapa_bombom_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige 3 argumentos (mapa<bombom,bombom>, bombom, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_definir'",
                ));
            };
            mapa.insert(key, value);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_bombom_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige 2 argumentos (mapa<bombom,bombom>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_obter'",
                ));
            };
            let Some(value) = mapa.get(&key) else {
                return Err(runtime_err("chave ausente em 'mapa_bombom_bombom_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "mapa_bombom_bombom_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige 2 argumentos (mapa<bombom,bombom>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(&key),
            ))))
        }
        "mapa_bombom_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tamanho' exige 1 argumento (mapa<bombom,bombom>)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tamanho' exige mapa<bombom,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_bombom_bombom_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige 2 argumentos (mapa, chave bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige mapa<bombom,bombom>",
                ));
            };
            let RuntimeValue::Int(chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige chave bombom",
                ));
            };
            let mapa = map_state
                .maps_bombom_bombom
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_bombom_bombom_remover': mapa inválido")
                })?;
            mapa.remove(&chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_bombom_bombom_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_criar' exige 1 argumento (mapa<bombom,bombom>)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_criar' exige mapa<bombom,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em '__pinker_internal_mapa_bombom_bombom_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_bombom_bombom.insert(
                iter_handle,
                RuntimeMapBombomBombomIter {
                    keys_snapshot: mapa.keys().copied().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_bombom_bombom.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave'",
                )
            })?;
            let key_val = *key;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(key_val))))
        }
        "mapa_bombom_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_bombom_verso.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapBombomVerso(
                handle,
            ))))
        }
        "mapa_bombom_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige 3 argumentos (mapa<bombom,verso>, bombom, verso)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Str(ref value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_definir'",
                ));
            };
            mapa.insert(key, value.clone());
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_bombom_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige 2 argumentos (mapa<bombom,verso>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_obter'",
                ));
            };
            let Some(value) = mapa.get(&key) else {
                return Err(runtime_err("chave ausente em 'mapa_bombom_verso_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "mapa_bombom_verso_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige 2 argumentos (mapa<bombom,verso>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(&key),
            ))))
        }
        "mapa_bombom_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tamanho' exige 1 argumento (mapa<bombom,verso>)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tamanho' exige mapa<bombom,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_bombom_verso_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige 2 argumentos (mapa, chave bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige mapa<bombom,verso>",
                ));
            };
            let RuntimeValue::Int(chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige chave bombom",
                ));
            };
            let mapa = map_state
                .maps_bombom_verso
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_bombom_verso_remover': mapa inválido")
                })?;
            mapa.remove(&chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_bombom_verso_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_criar' exige 1 argumento (mapa<bombom,verso>)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_criar' exige mapa<bombom,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em '__pinker_internal_mapa_bombom_verso_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_bombom_verso.insert(
                iter_handle,
                RuntimeMapBombomVersoIter {
                    keys_snapshot: mapa.keys().copied().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_bombom_verso.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave'",
                )
            })?;
            let key_val = *key;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(key_val))))
        }
        // @pinker-nav:end interpreter.intrinsecos.mapas-tipados

        // Arm isolado da família `listas` (ver `interpreter.intrinsecos.listas`),
        // fisicamente separado dela neste ponto do dispatcher; sem âncora própria.
        // Segue o ramo `_ => NotIntrinsic` de encerramento do dispatcher.
        "lista_bombom_inserir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige 3 argumentos (lista, índice bombom, valor bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige lista<bombom>",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige índice bombom",
                ));
            };
            let RuntimeValue::Int(valor) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige valor bombom",
                ));
            };
            let lista = list_state
                .lists_bombom
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'lista_bombom_inserir': lista inválida"))?;
            let idx = index as usize;
            if idx > lista.len() {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir': índice fora dos limites",
                ));
            }
            lista.insert(idx, valor);
            Ok(IntrinsicCall::Done(None))
        }
        _ => Ok(IntrinsicCall::NotIntrinsic),
    }
}

// @pinker-nav:start interpreter.hospedeiro.servicos-auxiliares
// @pinker-nav:domain hospedeiro
// @pinker-nav:layer interpreter
// @pinker-nav:summary Reúne helpers hospedados usados pelas intrínsecas para stdin, aleatoriedade, argumentos nomeados, ambiente, formatação textual, CSV, JSON mínimo, tempo UTC e processos; encapsula efeitos e normalizações auxiliares sem criar novas ferramentas da Trama nem alterar a semântica do dispatcher.
fn read_stdin_line_minima(intrinsic_name: &str) -> Result<Option<String>, PinkerError> {
    let mut raw = String::new();
    let bytes = io::stdin().read_line(&mut raw).map_err(|err| {
        runtime_err(&format!(
            "falha ao ler stdin em '{}': {}",
            intrinsic_name, err
        ))
    })?;
    if bytes == 0 {
        return Ok(None);
    }
    Ok(Some(raw))
}

fn advance_random_generator(state: &mut u64) -> u64 {
    // LCG mínimo e determinístico em u64, suficiente para o recorte auditável da fase.
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn ensure_named_arg_key_valid(intrinsic_name: &str, key: &str) -> Result<(), PinkerError> {
    if key.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige chave não vazia",
            intrinsic_name
        )));
    }
    Ok(())
}

fn ensure_env_key_valid(intrinsic_name: &str, key: &str) -> Result<(), PinkerError> {
    if key.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige chave de ambiente não vazia",
            intrinsic_name
        )));
    }
    Ok(())
}

fn formatar_verso_runtime(modelo: &str, args: &[RuntimeValue]) -> Result<String, PinkerError> {
    let mut saida = String::new();
    let mut ultimo_idx = 0usize;
    let mut arg_idx = 0usize;
    let mut chars = modelo.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        match ch {
            '{' => {
                saida.push_str(&modelo[ultimo_idx..idx]);
                let Some((close_idx, next_ch)) = chars.next() else {
                    return Err(runtime_err(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    ));
                };
                if next_ch != '}' {
                    return Err(runtime_err(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    ));
                }
                let Some(arg) = args.get(arg_idx) else {
                    return Err(runtime_err(
                        "quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos",
                    ));
                };
                saida.push_str(&formatar_verso_argumento(arg)?);
                arg_idx += 1;
                ultimo_idx = close_idx + next_ch.len_utf8();
            }
            '}' => {
                return Err(runtime_err(
                    "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                ));
            }
            _ => {}
        }
    }

    saida.push_str(&modelo[ultimo_idx..]);
    if arg_idx != args.len() {
        return Err(runtime_err(
            "quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos",
        ));
    }
    Ok(saida)
}

fn validar_separador_csv<'a>(
    intrinsic_name: &str,
    separador: &'a str,
) -> Result<&'a str, PinkerError> {
    if separador.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' não aceita separador vazio",
            intrinsic_name
        )));
    }
    if separador.chars().count() != 1 {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige separador de 1 caractere",
            intrinsic_name
        )));
    }
    if matches!(separador, "\"" | "\n" | "\r") {
        return Err(runtime_err(&format!(
            "intrínseca '{}' rejeita separador fora do recorte mínimo de CSV",
            intrinsic_name
        )));
    }
    Ok(separador)
}

fn parse_json_plano_bombom(json: &str) -> Result<HashMap<String, u64>, PinkerError> {
    let mut cursor = JsonPlanoCursor::new(json);
    cursor.skip_ws();
    cursor.expect_char('{')?;
    cursor.skip_ws();

    let mut mapa = HashMap::new();
    if cursor.consume_char('}') {
        cursor.skip_ws();
        cursor.ensure_eof()?;
        return Ok(mapa);
    }

    loop {
        cursor.skip_ws();
        let chave = cursor.parse_key()?;
        if mapa.contains_key(&chave) {
            return Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': chave duplicada fora do recorte auditável",
            ));
        }
        cursor.skip_ws();
        cursor.expect_char(':')?;
        cursor.skip_ws();
        let valor = cursor.parse_u64()?;
        mapa.insert(chave, valor);
        cursor.skip_ws();
        if cursor.consume_char('}') {
            cursor.skip_ws();
            cursor.ensure_eof()?;
            return Ok(mapa);
        }
        cursor.expect_char(',')?;
        cursor.skip_ws();
    }
}

fn emit_json_plano_bombom(mapa: &HashMap<String, u64>) -> Result<String, PinkerError> {
    let mut chaves = mapa.keys().cloned().collect::<Vec<_>>();
    chaves.sort();
    let mut partes = Vec::with_capacity(chaves.len());
    for chave in chaves {
        validar_chave_json_plana(&chave, "emitir_json_plano_bombom")?;
        let valor = mapa
            .get(&chave)
            .ok_or_else(|| runtime_err("mapa inconsistente em 'emitir_json_plano_bombom'"))?;
        partes.push(format!("\"{}\":{}", chave, valor));
    }
    Ok(format!("{{{}}}", partes.join(",")))
}

fn validar_chave_json_plana(chave: &str, nome: &str) -> Result<(), PinkerError> {
    if chave.contains('"') || chave.contains('\\') {
        return Err(runtime_err(&format!(
            "json inválido em '{}': chave exige escape fora do recorte",
            nome
        )));
    }
    if chave.chars().any(|ch| ch.is_control()) {
        return Err(runtime_err(&format!(
            "json inválido em '{}': chave contém controle fora do recorte",
            nome
        )));
    }
    Ok(())
}

struct JsonPlanoCursor<'a> {
    src: &'a str,
    idx: usize,
}

impl<'a> JsonPlanoCursor<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, idx: 0 }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.idx += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.idx..].chars().next()
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.idx += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), PinkerError> {
        if self.consume_char(expected) {
            Ok(())
        } else {
            Err(runtime_err(&format!(
                "json inválido em 'ler_json_plano_bombom': esperado '{}'",
                expected
            )))
        }
    }

    fn parse_key(&mut self) -> Result<String, PinkerError> {
        self.expect_char('"')?;
        let inicio = self.idx;
        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    let chave = self.src[inicio..self.idx].to_string();
                    self.idx += 1;
                    validar_chave_json_plana(&chave, "ler_json_plano_bombom")?;
                    return Ok(chave);
                }
                '\\' => {
                    return Err(runtime_err(
                        "json inválido em 'ler_json_plano_bombom': escapes em chave fora do recorte",
                    ));
                }
                _ if ch.is_control() => {
                    return Err(runtime_err(
                        "json inválido em 'ler_json_plano_bombom': controle em chave fora do recorte",
                    ));
                }
                _ => {
                    self.idx += ch.len_utf8();
                }
            }
        }
        Err(runtime_err(
            "json inválido em 'ler_json_plano_bombom': string de chave não terminada",
        ))
    }

    fn parse_u64(&mut self) -> Result<u64, PinkerError> {
        let inicio = self.idx;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.idx += ch.len_utf8();
            } else {
                break;
            }
        }
        if inicio == self.idx {
            return Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': valor deve ser bombom sem sinal",
            ));
        }
        self.src[inicio..self.idx].parse::<u64>().map_err(|_| {
            runtime_err("json inválido em 'ler_json_plano_bombom': bombom fora da faixa")
        })
    }

    fn ensure_eof(&self) -> Result<(), PinkerError> {
        if self.idx == self.src.len() {
            Ok(())
        } else {
            Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': conteúdo extra após objeto",
            ))
        }
    }
}

fn formatar_verso_argumento(arg: &RuntimeValue) -> Result<String, PinkerError> {
    match arg {
        RuntimeValue::Int(value) => Ok(value.to_string()),
        RuntimeValue::Str(value) => Ok(value.clone()),
        _ => Err(runtime_err(
            "intrínseca 'formatar_verso' exige argumentos de substituição em bombom ou verso",
        )),
    }
}

fn formatar_tempo_unix_iso_utc(timestamp: u64) -> Result<String, PinkerError> {
    let dias = timestamp / 86_400;
    let segundos_do_dia = timestamp % 86_400;
    let dias = i64::try_from(dias).map_err(|_| {
        runtime_err("timestamp inválido em 'formatar_tempo_unix': fora da faixa suportada")
    })?;
    let (ano, mes, dia) = civil_from_days(dias)?;
    let hora = segundos_do_dia / 3_600;
    let minuto = (segundos_do_dia % 3_600) / 60;
    let segundo = segundos_do_dia % 60;
    Ok(format!(
        "{ano:04}-{mes:02}-{dia:02}T{hora:02}:{minuto:02}:{segundo:02}Z"
    ))
}

fn executar_processo_minimo(
    command_name: &str,
    explicit_argv: Option<&str>,
) -> Result<u64, PinkerError> {
    validar_comando_nao_vazio("executar_processo", command_name)?;

    let mut command = Command::new(command_name);
    if let Some(arg) = explicit_argv {
        command.arg(arg);
    }

    let status = command.status().map_err(|err| {
        runtime_err(&format!(
            "falha ao executar processo em 'executar_processo': {}",
            err
        ))
    })?;

    exit_code_u64("executar_processo", status.code())
}

fn executar_com_entrada_minimo(
    command_name: &str,
    input_text: &str,
    explicit_argv: Option<&str>,
) -> Result<u64, PinkerError> {
    validar_comando_nao_vazio("executar_com_entrada", command_name)?;

    let mut command = Command::new(command_name);
    if let Some(arg) = explicit_argv {
        command.arg(arg);
    }

    let mut child = command.stdin(Stdio::piped()).spawn().map_err(|err| {
        runtime_err(&format!(
            "falha ao executar processo em 'executar_com_entrada': {}",
            err
        ))
    })?;

    let mut stdin = child.stdin.take().ok_or_else(|| {
        runtime_err("stdin indisponível em 'executar_com_entrada': processo sem pipe configurado")
    })?;
    stdin.write_all(input_text.as_bytes()).map_err(|err| {
        runtime_err(&format!(
            "falha ao escrever stdin em 'executar_com_entrada': {}",
            err
        ))
    })?;
    drop(stdin);

    let status = child.wait().map_err(|err| {
        runtime_err(&format!(
            "falha ao aguardar processo em 'executar_com_entrada': {}",
            err
        ))
    })?;

    exit_code_u64("executar_com_entrada", status.code())
}

fn pipeline_minimo(producer_name: &str, consumer_name: &str) -> Result<u64, PinkerError> {
    validar_comando_nao_vazio("pipeline_minimo", producer_name)?;
    validar_comando_nao_vazio("pipeline_minimo", consumer_name)?;

    let mut producer = Command::new(producer_name)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| {
            runtime_err(&format!(
                "falha ao executar processo produtor em 'pipeline_minimo': {}",
                err
            ))
        })?;

    let producer_stdout = producer.stdout.take().ok_or_else(|| {
        runtime_err("stdout indisponível em 'pipeline_minimo': produtor sem pipe configurado")
    })?;

    let mut consumer = Command::new(consumer_name)
        .stdin(Stdio::from(producer_stdout))
        .spawn()
        .map_err(|err| {
            runtime_err(&format!(
                "falha ao executar processo consumidor em 'pipeline_minimo': {}",
                err
            ))
        })?;

    let consumer_status = consumer.wait().map_err(|err| {
        runtime_err(&format!(
            "falha ao aguardar processo consumidor em 'pipeline_minimo': {}",
            err
        ))
    })?;

    producer.wait().map_err(|err| {
        runtime_err(&format!(
            "falha ao aguardar processo produtor em 'pipeline_minimo': {}",
            err
        ))
    })?;

    exit_code_u64("pipeline_minimo", consumer_status.code())
}

fn capturar_stdout_minimo(
    command_name: &str,
    explicit_argv: Option<&str>,
) -> Result<String, PinkerError> {
    validar_comando_nao_vazio("capturar_stdout", command_name)?;

    let mut command = Command::new(command_name);
    if let Some(arg) = explicit_argv {
        command.arg(arg);
    }

    let output = command.output().map_err(|err| {
        runtime_err(&format!(
            "falha ao executar processo em 'capturar_stdout': {}",
            err
        ))
    })?;

    String::from_utf8(output.stdout).map_err(|_| {
        runtime_err("stdout inválido em 'capturar_stdout': UTF-8 estrito é obrigatório")
    })
}

fn capturar_stderr_minimo(
    command_name: &str,
    explicit_argv: Option<&str>,
) -> Result<String, PinkerError> {
    validar_comando_nao_vazio("capturar_stderr", command_name)?;

    let mut command = Command::new(command_name);
    if let Some(arg) = explicit_argv {
        command.arg(arg);
    }

    let output = command.output().map_err(|err| {
        runtime_err(&format!(
            "falha ao executar processo em 'capturar_stderr': {}",
            err
        ))
    })?;

    String::from_utf8(output.stderr).map_err(|_| {
        runtime_err("stderr inválido em 'capturar_stderr': UTF-8 estrito é obrigatório")
    })
}

fn validar_comando_nao_vazio(intrinsic_name: &str, command_name: &str) -> Result<(), PinkerError> {
    if command_name.trim().is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige comando não vazio",
            intrinsic_name
        )));
    }
    Ok(())
}

fn exit_code_u64(intrinsic_name: &str, exit_code: Option<i32>) -> Result<u64, PinkerError> {
    let exit_code = exit_code.ok_or_else(|| {
        runtime_err(&format!(
            "processo finalizado sem código de saída suportado em '{}'",
            intrinsic_name
        ))
    })?;

    u64::try_from(exit_code).map_err(|_| {
        runtime_err(&format!(
            "código de saída inválido em '{}': valor negativo",
            intrinsic_name
        ))
    })
}

fn civil_from_days(days_since_unix_epoch: i64) -> Result<(i64, u64, u64), PinkerError> {
    let z = days_since_unix_epoch.checked_add(719_468).ok_or_else(|| {
        runtime_err("timestamp inválido em 'formatar_tempo_unix': fora da faixa suportada")
    })?;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Ok((
        year,
        u64::try_from(month).map_err(|_| {
            runtime_err("timestamp inválido em 'formatar_tempo_unix': mês fora da faixa")
        })?,
        u64::try_from(day).map_err(|_| {
            runtime_err("timestamp inválido em 'formatar_tempo_unix': dia fora da faixa")
        })?,
    ))
}

fn find_named_cli_argument<'a>(args: &'a [String], key: &str) -> NamedArgLookup<'a> {
    let key_eq = format!("{key}=");
    for (index, arg) in args.iter().enumerate() {
        if arg == key {
            return match args.get(index + 1) {
                Some(value) => NamedArgLookup::PresentValue(value),
                None => NamedArgLookup::PresentWithoutValue,
            };
        }
        if let Some(value) = arg.strip_prefix(&key_eq) {
            return NamedArgLookup::PresentValue(value);
        }
    }
    NamedArgLookup::Missing
}

fn trim_final_newline_minimo(mut line: String) -> String {
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    line
}

// @pinker-nav:end interpreter.hospedeiro.servicos-auxiliares

// @pinker-nav:start interpreter.execucao.valores-tipos
// @pinker-nav:domain execucao
// @pinker-nav:layer interpreter
// @pinker-nav:summary Implementa busca de função, desempilhamento de argumentos, validações dinâmicas de tipo, coerção para `TypeIR`, conversões de ponteiros simulados, aritmética, comparação e signedness usados pela execução; são defesas de runtime, não o sistema estático de tipos Pinker.
fn find_function<'a>(
    name: &str,
    program: &'a MachineProgram,
) -> Result<&'a MachineFunction, PinkerError> {
    program
        .functions
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| runtime_err("função chamada inexistente"))
}

// Desempilha `argc` argumentos e reverte a ordem para corresponder à
// declaração da função (pilha é LIFO, mas args foram empilhados left-to-right).
fn pop_args(stack: &mut Vec<RuntimeValue>, argc: usize) -> Result<Vec<RuntimeValue>, PinkerError> {
    let mut args = Vec::with_capacity(argc);
    for _ in 0..argc {
        args.push(pop(stack, "underflow em argumentos de chamada")?);
    }
    args.reverse();
    Ok(args)
}

fn pop(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<RuntimeValue, PinkerError> {
    stack.pop().ok_or_else(|| runtime_err(msg))
}

fn pop_numeric(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<RuntimeValue, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Int(v) => Ok(RuntimeValue::Int(v)),
        RuntimeValue::IntSigned(v) => Ok(RuntimeValue::IntSigned(v)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
        RuntimeValue::Bool(_) => Err(runtime_err(msg)),
        RuntimeValue::Str(_) => Err(runtime_err(msg)),
        RuntimeValue::ListBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::ListVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::MapBombomBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapBombomVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::Callable(_) => Err(runtime_err(msg)),
    }
}

fn pop_bool(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<bool, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Bool(v) => Ok(v),
        RuntimeValue::Int(_) => Err(runtime_err(msg)),
        RuntimeValue::IntSigned(_) => Err(runtime_err(msg)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
        RuntimeValue::Str(_) => Err(runtime_err(msg)),
        RuntimeValue::ListBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::ListVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::MapBombomBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapBombomVerso(_) => Err(runtime_err(msg)),
        RuntimeValue::Callable(_) => Err(runtime_err(msg)),
    }
}

fn pop_str(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<String, PinkerError> {
    let value = pop(stack, msg)?;
    match value {
        RuntimeValue::Str(v) => Ok(v),
        _ => Err(runtime_err(msg)),
    }
}

fn pop_bin_numeric(
    stack: &mut Vec<RuntimeValue>,
    msg: &str,
) -> Result<(RuntimeValue, RuntimeValue), PinkerError> {
    let rhs = pop_numeric(stack, msg)?;
    let lhs = pop_numeric(stack, msg)?;
    Ok((lhs, rhs))
}

fn integer_width_bits(ty: crate::ir::TypeIR) -> Option<u32> {
    match ty {
        crate::ir::TypeIR::U8 | crate::ir::TypeIR::I8 => Some(8),
        crate::ir::TypeIR::U16 | crate::ir::TypeIR::I16 => Some(16),
        crate::ir::TypeIR::U32 | crate::ir::TypeIR::I32 => Some(32),
        crate::ir::TypeIR::U64 | crate::ir::TypeIR::I64 | crate::ir::TypeIR::Bombom => Some(64),
        _ => None,
    }
}

fn integer_raw_bits(value: RuntimeValue) -> Result<u64, PinkerError> {
    match value {
        RuntimeValue::Int(value) => Ok(value),
        RuntimeValue::IntSigned(value) => Ok(value as u64),
        RuntimeValue::Ptr(value) => Ok(value as u64),
        _ => Err(runtime_err("operação inteira exige valor numérico")),
    }
}

fn normalize_integer(
    value: RuntimeValue,
    ty: crate::ir::TypeIR,
) -> Result<RuntimeValue, PinkerError> {
    let width = integer_width_bits(ty)
        .ok_or_else(|| runtime_err("normalização inteira recebeu tipo não inteiro"))?;
    let raw = integer_raw_bits(value)?;
    let masked = if width == 64 {
        raw
    } else {
        raw & ((1u64 << width) - 1)
    };
    if ty.is_signed() {
        let signed = if width == 64 {
            masked as i64
        } else {
            ((masked << (64 - width)) as i64) >> (64 - width)
        };
        Ok(RuntimeValue::IntSigned(signed))
    } else {
        Ok(RuntimeValue::Int(masked))
    }
}

fn normalize_comparison_pair(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    ty: crate::ir::TypeIR,
) -> Result<(RuntimeValue, RuntimeValue), PinkerError> {
    if ty.is_integer() {
        Ok((normalize_integer(lhs, ty)?, normalize_integer(rhs, ty)?))
    } else {
        Ok((lhs, rhs))
    }
}

fn eval_shift(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    ty: crate::ir::TypeIR,
    right: bool,
) -> Result<RuntimeValue, PinkerError> {
    let width = integer_width_bits(ty)
        .ok_or_else(|| runtime_err("shift recebeu tipo operacional não inteiro"))?;
    let count = match rhs {
        RuntimeValue::Int(value) => value,
        RuntimeValue::IntSigned(value) if value >= 0 => value as u64,
        RuntimeValue::IntSigned(_) => {
            return Err(runtime_err(
                "E-RUNTIME-SHIFT-COUNT: contagem de shift deve ser não negativa",
            ));
        }
        _ => return Err(runtime_err("shift exige contagem inteira")),
    };
    if count >= u64::from(width) {
        return Err(runtime_err(&format!(
            "E-RUNTIME-SHIFT-COUNT: contagem {count} fora da largura {width}"
        )));
    }

    let lhs = normalize_integer(lhs, ty)?;
    let shifted = if right && ty.is_signed() {
        let RuntimeValue::IntSigned(value) = lhs else {
            unreachable!("normalização signed produz IntSigned")
        };
        RuntimeValue::IntSigned(value >> (count as u32))
    } else {
        let raw = integer_raw_bits(lhs)?;
        let value = if right {
            raw >> (count as u32)
        } else {
            raw << (count as u32)
        };
        RuntimeValue::Int(value)
    };
    normalize_integer(shifted, ty)
}

fn coerce_runtime_value_to_type(
    value: RuntimeValue,
    ty: crate::ir::TypeIR,
) -> Result<RuntimeValue, PinkerError> {
    if ty.is_integer() {
        return match value {
            value @ (RuntimeValue::Int(_) | RuntimeValue::IntSigned(_)) => {
                normalize_integer(value, ty)
            }
            // Handles opacos históricos (ninhos, arrays e leques com carga)
            // atravessam alguns slots `bombom` como ponteiros. Normalização
            // numérica de armazenamento não pode apagar essa categoria; um
            // cast público ponteiro→inteiro continua proibido no semantic.
            value @ RuntimeValue::Ptr(_) => Ok(value),
            RuntimeValue::Str(_) => Err(runtime_err("cast inteiro não aceita verso")),
            RuntimeValue::ListBombom(_) => {
                Err(runtime_err("cast inteiro não aceita lista<bombom>"))
            }
            RuntimeValue::ListVerso(_) => Err(runtime_err("cast inteiro não aceita lista<verso>")),
            RuntimeValue::MapVersoBombom(_) => {
                Err(runtime_err("cast inteiro não aceita mapa<verso,bombom>"))
            }
            RuntimeValue::MapVersoVerso(_) => {
                Err(runtime_err("cast inteiro não aceita mapa<verso,verso>"))
            }
            RuntimeValue::MapBombomBombom(_) => {
                Err(runtime_err("cast inteiro não aceita mapa<bombom,bombom>"))
            }
            RuntimeValue::MapBombomVerso(_) => {
                Err(runtime_err("cast inteiro não aceita mapa<bombom,verso>"))
            }
            other => Ok(other),
        };
    }

    if let crate::ir::TypeIR::Union(expected_id) = ty {
        return match value {
            RuntimeValue::Ptr(handle) => UNION_RUNTIME_STATE.with(|state| {
                let state = state.borrow();
                let descriptor = state
                    .descriptors
                    .get(&handle)
                    .ok_or_else(|| runtime_err("handle de união inexistente"))?;
                if descriptor.union_type_id != expected_id {
                    return Err(runtime_err("handle pertence a outro tipo de união"));
                }
                Ok(RuntimeValue::Ptr(handle))
            }),
            _ => Err(runtime_err("valor incompatível: esperado união estrutural")),
        };
    }

    if matches!(
        ty,
        crate::ir::TypeIR::Pointer { .. } | crate::ir::TypeIR::FunctionPointer
    ) {
        return match value {
            RuntimeValue::Int(v) => Ok(RuntimeValue::Ptr(v as usize)),
            RuntimeValue::IntSigned(v) if v < 0 => Err(runtime_err(
                "endereço de ponteiro inválido em runtime: valor negativo",
            )),
            RuntimeValue::IntSigned(v) => Ok(RuntimeValue::Ptr(v as usize)),
            RuntimeValue::Ptr(v) => Ok(RuntimeValue::Ptr(v)),
            RuntimeValue::Bool(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::Str(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::ListBombom(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::ListVerso(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::MapVersoBombom(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::MapVersoVerso(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::MapBombomBombom(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::MapBombomVerso(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::Callable(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
        };
    }

    if matches!(ty, crate::ir::TypeIR::ListBombom) {
        return match value {
            RuntimeValue::ListBombom(handle) => Ok(RuntimeValue::ListBombom(handle)),
            _ => Err(runtime_err("valor incompatível: esperado lista<bombom>")),
        };
    }
    if matches!(ty, crate::ir::TypeIR::ListVerso) {
        return match value {
            RuntimeValue::ListVerso(handle) => Ok(RuntimeValue::ListVerso(handle)),
            _ => Err(runtime_err("valor incompatível: esperado lista<verso>")),
        };
    }
    if matches!(ty, crate::ir::TypeIR::MapVersoBombom) {
        return match value {
            RuntimeValue::MapVersoBombom(handle) => Ok(RuntimeValue::MapVersoBombom(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado mapa<verso,bombom>",
            )),
        };
    }
    if matches!(ty, crate::ir::TypeIR::MapVersoVerso) {
        return match value {
            RuntimeValue::MapVersoVerso(handle) => Ok(RuntimeValue::MapVersoVerso(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado mapa<verso,verso>",
            )),
        };
    }
    if matches!(ty, crate::ir::TypeIR::MapBombomBombom) {
        return match value {
            RuntimeValue::MapBombomBombom(handle) => Ok(RuntimeValue::MapBombomBombom(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado mapa<bombom,bombom>",
            )),
        };
    }
    if matches!(ty, crate::ir::TypeIR::MapBombomVerso) {
        return match value {
            RuntimeValue::MapBombomVerso(handle) => Ok(RuntimeValue::MapBombomVerso(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado mapa<bombom,verso>",
            )),
        };
    }

    Ok(value)
}

fn current_function<'a>(
    program: &'a MachineProgram,
    call_stack: &[RuntimeFrame],
) -> Result<&'a MachineFunction, PinkerError> {
    let fn_name = call_stack
        .last()
        .map(|frame| frame.fn_name.as_str())
        .ok_or_else(|| runtime_err("pilha de chamadas vazia"))?;
    find_function(fn_name, program)
}

fn bin_int(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    op_u: fn(u64, u64) -> u64,
    op_s: fn(i64, i64) -> i64,
) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => Ok(RuntimeValue::Int(op_u(a, b))),
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            Ok(RuntimeValue::IntSigned(op_s(a, b)))
        }
        _ => Err(runtime_err("operação inteira inválida em runtime")),
    }
}

fn eval_add(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match (lhs, rhs) {
        (RuntimeValue::Ptr(base), RuntimeValue::Int(offset)) => {
            Ok(RuntimeValue::Ptr(base.wrapping_add(offset as usize)))
        }
        (lhs, rhs) => bin_int(lhs, rhs, |a, b| a.wrapping_add(b), |a, b| a.wrapping_add(b))
            .map_err(|_| runtime_err("add exige inteiros ou 'seta<bombom> + bombom'")),
    }
}

fn eval_sub(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match (lhs, rhs) {
        (RuntimeValue::Ptr(base), RuntimeValue::Int(offset)) => {
            Ok(RuntimeValue::Ptr(base.wrapping_sub(offset as usize)))
        }
        (lhs, rhs) => bin_int(lhs, rhs, |a, b| a.wrapping_sub(b), |a, b| a.wrapping_sub(b))
            .map_err(|_| runtime_err("sub exige inteiros ou 'seta<bombom> - bombom'")),
    }
}

fn cmp_int(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    op_u: fn(u64, u64) -> bool,
    op_s: fn(i64, i64) -> bool,
) -> Result<bool, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => Ok(op_u(a, b)),
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => Ok(op_s(a, b)),
        _ => Err(runtime_err("comparação inteira inválida em runtime")),
    }
}

fn bin_int_checked_div(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::Int(a / b))
        }
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            if a == i64::MIN && b == -1 {
                return Ok(RuntimeValue::IntSigned(i64::MIN));
            }
            Ok(RuntimeValue::IntSigned(a / b))
        }
        _ => Err(runtime_err("divisão inteira inválida em runtime")),
    }
}

fn bin_int_checked_mod(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::Int(a % b))
        }
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            if a == i64::MIN && b == -1 {
                return Ok(RuntimeValue::IntSigned(0));
            }
            Ok(RuntimeValue::IntSigned(a % b))
        }
        _ => Err(runtime_err("módulo inteiro inválido em runtime")),
    }
}

fn normalize_numeric_pair(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
) -> Result<(RuntimeValue, RuntimeValue), PinkerError> {
    match (&lhs, &rhs) {
        (RuntimeValue::Int(_), RuntimeValue::Int(_))
        | (RuntimeValue::IntSigned(_), RuntimeValue::IntSigned(_)) => Ok((lhs, rhs)),
        // lhs signed, rhs unsigned: converte rhs para signed preservando ordem
        (RuntimeValue::IntSigned(a), RuntimeValue::Int(b)) => {
            if *b > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(*a),
                RuntimeValue::IntSigned(*b as i64),
            ))
        }
        // lhs unsigned, rhs signed: converte lhs para signed preservando ordem
        (RuntimeValue::Int(a), RuntimeValue::IntSigned(b)) => {
            if *a > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(*a as i64),
                RuntimeValue::IntSigned(*b),
            ))
        }
        _ => Err(runtime_err("operação inteira exige valores inteiros")),
    }
}

// @pinker-nav:end interpreter.execucao.valores-tipos

// @pinker-nav:start interpreter.diagnostico.stack-trace
// @pinker-nav:domain diagnostico
// @pinker-nav:layer interpreter
// @pinker-nav:summary Cria erros de runtime enriquecidos e stack traces do interpretador a partir dos frames Pinker ativos, incluindo função, bloco, instrução e span futuro quando disponível, prevenindo anexação duplicada e truncando traces longos; não é backtrace nativo Rust.
fn runtime_err(msg: &str) -> PinkerError {
    PinkerError::Runtime {
        msg: enrich_runtime_msg(msg),
        span: None,
    }
}

fn deref_load_normal(memory: &HashMap<usize, RuntimeValue>, addr: usize) -> Option<RuntimeValue> {
    memory.get(&addr).cloned()
}

fn deref_load_fragil(memory: &HashMap<usize, RuntimeValue>, addr: usize) -> Option<RuntimeValue> {
    memory.get(&addr).cloned()
}

fn deref_store_normal(memory: &mut HashMap<usize, RuntimeValue>, addr: usize, value: RuntimeValue) {
    memory.insert(addr, value);
}

fn deref_store_fragil(memory: &mut HashMap<usize, RuntimeValue>, addr: usize, value: RuntimeValue) {
    memory.insert(addr, value);
}

fn enrich_runtime_msg(msg: &str) -> String {
    let (kind, hint) = classify_runtime_msg(msg);
    format!(
        "[runtime::{kind}] {msg}{}",
        hint.map(|h| format!(" | dica: {h}")).unwrap_or_default()
    )
}

fn classify_runtime_msg(msg: &str) -> (&'static str, Option<&'static str>) {
    if msg.contains("limite preventivo de recursão excedido") {
        (
            "limite_recursao_excedido",
            Some(
                "revise o caso-base da função recursiva para garantir término antes do limite interno",
            ),
        )
    } else if msg.contains("divisão por zero") {
        (
            "divisao_por_zero",
            Some("verifique se o divisor é diferente de 0 antes da operação '/'"),
        )
    } else if msg.contains("slot não inicializado") {
        (
            "slot_nao_inicializado",
            Some("inicialize o slot antes de fazer load_slot"),
        )
    } else if msg.contains("função chamada inexistente") {
        (
            "funcao_inexistente",
            Some("confira se o nome da função e a assinatura existem no programa"),
        )
    } else if msg.contains("aridade inválida") {
        (
            "aridade_invalida",
            Some("confira a quantidade de argumentos passados na chamada"),
        )
    } else if msg.contains("handle já fechado") {
        (
            "handle_ja_fechado",
            Some("o handle já foi fechado com 'fechar'; abra novamente com 'abrir', 'criar_arquivo' ou 'abrir_anexo' se necessário"),
        )
    } else if msg.contains("global inexistente") {
        (
            "global_inexistente",
            Some("use apenas globals declaradas em `eterno`"),
        )
    } else if msg.contains("deref_load")
        || msg.contains("deref_store")
        || msg.contains("endereço inválido")
        || msg.contains("ponteiro no topo")
    {
        (
            "acesso_invalido_ptr",
            Some("verifique se o endereço do ponteiro está mapeado (global escalar declarada)"),
        )
    } else {
        ("erro", None)
    }
}

// Adiciona o stack trace textual à mensagem de erro, se ainda não tiver sido
// adicionado (evita duplicação quando o erro borbulha por múltiplos frames).
fn attach_runtime_trace(err: PinkerError, call_stack: &[RuntimeFrame]) -> PinkerError {
    match err {
        PinkerError::Runtime { msg, span } => {
            if msg.contains("\nstack trace:\n") {
                PinkerError::Runtime { msg, span }
            } else {
                let mut traced = msg;
                traced.push_str(&render_runtime_trace(call_stack));
                PinkerError::Runtime { msg: traced, span }
            }
        }
        _ => err,
    }
}

fn render_frame(frame: &RuntimeFrame, out: &mut String) {
    out.push_str("  at ");
    out.push_str(&frame.fn_name);
    if let Some(label) = &frame.block_label {
        out.push_str(" [bloco: ");
        out.push_str(label);
        out.push(']');
    }
    if let Some(instr) = frame.current_instr {
        out.push_str(" [instr: ");
        out.push_str(instr);
        out.push(']');
    }
    if let Some(span) = frame.future_span {
        out.push_str(" [span: ");
        out.push_str(&span.to_string());
        out.push(']');
    }
    out.push('\n');
}

fn render_runtime_trace(call_stack: &[RuntimeFrame]) -> String {
    let mut out = String::from("\nstack trace:\n");
    let n = call_stack.len();
    if n <= TRACE_TRUNC_THRESHOLD {
        for frame in call_stack {
            render_frame(frame, &mut out);
        }
    } else {
        for frame in &call_stack[..TRACE_HEAD] {
            render_frame(frame, &mut out);
        }
        let omitted = n - TRACE_HEAD - TRACE_TAIL;
        out.push_str(&format!("  ... {omitted} frames omitidos ...\n"));
        for frame in &call_stack[n - TRACE_TAIL..] {
            render_frame(frame, &mut out);
        }
    }
    out
}

fn set_current_instr(call_stack: &mut [RuntimeFrame], instr_name: Option<&'static str>) {
    if let Some(frame) = call_stack.last_mut() {
        frame.current_instr = instr_name;
    }
}

fn machine_instr_name(instr: &MachineInstr) -> &'static str {
    match instr {
        MachineInstr::PushInt(_) => "push_int",
        MachineInstr::PushBool(_) => "push_bool",
        MachineInstr::PushStr(_) => "push_str",
        MachineInstr::LoadSlot(_) => "load_slot",
        MachineInstr::LoadGlobal(_) => "load_global",
        MachineInstr::StoreSlot(_) => "store_slot",
        MachineInstr::Neg { .. } => "neg",
        MachineInstr::Not => "not",
        MachineInstr::BitNot { .. } => "bitnot",
        MachineInstr::DerefLoad { is_volatile, .. } => {
            if *is_volatile {
                "deref_load_fragil"
            } else {
                "deref_load"
            }
        }
        MachineInstr::DerefStore { is_volatile, .. } => {
            if *is_volatile {
                "deref_store_fragil"
            } else {
                "deref_store"
            }
        }
        MachineInstr::Cast { .. } => "cast",
        MachineInstr::MakeUnion { .. } => "make_union",
        MachineInstr::BitAnd { .. } => "bitand",
        MachineInstr::BitOr { .. } => "bitor",
        MachineInstr::BitXor { .. } => "bitxor",
        MachineInstr::Shl { .. } => "shl",
        MachineInstr::Shr { .. } => "shr",
        MachineInstr::Add { .. } => "add",
        MachineInstr::Sub { .. } => "sub",
        MachineInstr::Mul { .. } => "mul",
        MachineInstr::Div { .. } => "div",
        MachineInstr::Mod { .. } => "mod",
        MachineInstr::CmpEq { .. } => "cmp_eq",
        MachineInstr::CmpNe { .. } => "cmp_ne",
        MachineInstr::CmpLt { .. } => "cmp_lt",
        MachineInstr::CmpLe { .. } => "cmp_le",
        MachineInstr::CmpGt { .. } => "cmp_gt",
        MachineInstr::CmpGe { .. } => "cmp_ge",
        MachineInstr::Call { .. } => "call",
        MachineInstr::CallVoid { .. } => "call_void",
        MachineInstr::PushFunctionRef(_) => "push_function_ref",
        MachineInstr::PushRawFunctionRef(_) => "push_raw_function_ref",
        MachineInstr::CallIndirect { .. } => "call_indirect",
        MachineInstr::CallRaw { .. } => "call_raw",
        MachineInstr::MakeClosure { .. } => "make_closure",
        MachineInstr::MakeTraitObject { .. } => "make_trait_object",
        MachineInstr::TraitCall { .. } => "trait_call",
        MachineInstr::PrintIntInline => "print_int_inline",
        MachineInstr::PrintBoolInline => "print_bool_inline",
        MachineInstr::PrintStrValueInline => "print_str_value_inline",
        MachineInstr::PrintStrInline(_) => "print_str_inline",
        MachineInstr::PrintSpace => "print_space",
        MachineInstr::PrintNewline => "print_newline",
        MachineInstr::InlineAsm { .. } => "inline_asm",
    }
}
// @pinker-nav:end interpreter.diagnostico.stack-trace

#[cfg(test)]
mod fase244_trait_runtime_tests {
    use super::*;

    #[test]
    fn fase244_trait_runtime_snapshot_composto_independe_da_origem() {
        let mut state = TraitObjectState::new();
        let mut memory = HashMap::new();

        let source_addr = 0x4000usize;
        memory.insert(source_addr, RuntimeValue::Int(11));
        memory.insert(source_addr + 8, RuntimeValue::Int(22));

        let methods = vec!["__impl_7_Medivel_5_Ponto_medir".to_string()];

        let handle = state
            .create_object(
                RuntimeValue::Ptr(source_addr),
                "Medivel",
                crate::ir::TypeIR::Struct,
                "Ponto",
                16,
                &methods,
                &mut memory,
            )
            .unwrap();

        let (_, receiver) = state
            .resolve_call(handle, "Medivel", "medir", 0, &memory)
            .unwrap();

        let RuntimeValue::Ptr(snapshot_addr) = receiver else {
            panic!("receiver composto deveria ser ponteiro");
        };

        assert_ne!(snapshot_addr, source_addr);
        assert_eq!(memory.get(&snapshot_addr), Some(&RuntimeValue::Int(11)));
        assert_eq!(
            memory.get(&(snapshot_addr + 8)),
            Some(&RuntimeValue::Int(22))
        );

        memory.insert(source_addr, RuntimeValue::Int(99));
        memory.insert(source_addr + 8, RuntimeValue::Int(100));

        assert_eq!(memory.get(&snapshot_addr), Some(&RuntimeValue::Int(11)));
        assert_eq!(
            memory.get(&(snapshot_addr + 8)),
            Some(&RuntimeValue::Int(22))
        );
    }

    #[test]
    fn fase244_trait_runtime_vtable_e_internada_handles_sao_distintos() {
        let mut state = TraitObjectState::new();
        let mut memory = HashMap::new();
        let methods = vec!["__impl_7_Medivel_6_bombom_medir".to_string()];

        let first = state
            .create_object(
                RuntimeValue::Int(10),
                "Medivel",
                crate::ir::TypeIR::Bombom,
                "bombom",
                8,
                &methods,
                &mut memory,
            )
            .unwrap();

        let second = state
            .create_object(
                RuntimeValue::Int(20),
                "Medivel",
                crate::ir::TypeIR::Bombom,
                "bombom",
                8,
                &methods,
                &mut memory,
            )
            .unwrap();

        assert_ne!(first, second);

        let first_vtable = state.table.get(&first).unwrap().vtable_handle;
        let second_vtable = state.table.get(&second).unwrap().vtable_handle;

        assert_eq!(first_vtable, second_vtable);

        // Copiar o valor público copia apenas o handle e, portanto,
        // continua apontando para o mesmo descritor.
        let alias = first;
        assert_eq!(
            state.table.get(&alias).unwrap().data_addr,
            state.table.get(&first).unwrap().data_addr
        );
    }
}

#[cfg(test)]
mod fase246_public_memory_tests {
    use super::*;

    fn registrar_regiao(state: &mut PublicMemoryState, base: usize, size: usize, alive: bool) {
        state.regions.push(PublicMemoryRegion { base, size, alive });
    }

    #[test]
    fn liberar_endereco_reutilizado_escolhe_a_geracao_viva_mais_recente() {
        let mut state = PublicMemoryState::default();
        let base = 0x6000_0000;
        registrar_regiao(&mut state, base, 16, false);
        registrar_regiao(&mut state, base, 16, false);
        registrar_regiao(&mut state, base, 16, true);

        public_memory_free(&[RuntimeValue::Ptr(base)], &mut state)
            .expect("a terceira geração viva deve poder ser liberada");

        assert!(!state.regions[0].alive);
        assert!(!state.regions[1].alive);
        assert!(!state.regions[2].alive);
    }

    #[test]
    fn bytes_publicos_preservam_largura_aliasing_e_extensao() {
        let mut memory = HashMap::new();
        let base = 0x6000_1000;

        public_memory_store_bytes(
            &mut memory,
            base,
            TypeIR::U32,
            RuntimeValue::Int(0x1234_5678),
        )
        .expect("store u32");
        assert_eq!(
            public_memory_load_bytes(&memory, base, TypeIR::U8).expect("load u8"),
            RuntimeValue::Int(0x78)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base, TypeIR::U16).expect("load u16"),
            RuntimeValue::Int(0x5678)
        );

        public_memory_store_bytes(
            &mut memory,
            base + 4,
            TypeIR::I8,
            RuntimeValue::IntSigned(-128),
        )
        .expect("store i8");
        assert_eq!(
            public_memory_load_bytes(&memory, base + 4, TypeIR::I8).expect("load i8"),
            RuntimeValue::IntSigned(-128)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base + 4, TypeIR::U8).expect("load u8"),
            RuntimeValue::Int(128)
        );
        assert_eq!(
            public_memory_load_bytes(&memory, base + 8, TypeIR::U64).expect("zero u64"),
            RuntimeValue::Int(0)
        );
    }
}
