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
use crate::falha_operacional::{OperacaoFalivel, SuperficieFalivel};
use crate::ir::TypeIR;
use crate::token::Span;
use pinker_memory_contract::{
    release_public_live_bytes, reserve_public_allocation, PublicAllocationVerdict,
    PublicMemoryBudget, PublicMemoryLimits as ContractMemoryLimits,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
mod hosted_intrinsics;
use hosted_intrinsics::try_call_intrinsic;

/// Snapshot imutável do payload de uma união (HR3).
///
/// O descritor **nunca** referencia o storage do chamador: escalares e handles
/// são clonados no ato da injeção, e agregados são copiados byte a byte para
/// este snapshot. Mudar a origem depois da injeção não muda o que o `encaixe`
/// observa.
#[derive(Clone, PartialEq, Eq)]
enum UnionPayloadSnapshot {
    /// Inteiro, lógico ou leque sem carga: valor por cópia.
    Scalar(RuntimeValue),
    /// Handle de uma palavra. A cópia é rasa **por contrato**: o descritor
    /// apontado por um `verso`, lista, mapa ou callable dentro da união é o
    /// mesmo objeto, exatamente como no runtime nativo.
    OpaqueHandle(RuntimeValue),
    /// Agregado copiado integralmente. Os bytes são a representação exata da
    /// origem, incluindo padding.
    Aggregate { bytes: Vec<u8> },
}

#[derive(Clone)]
struct UnionRuntimeDescriptor {
    union_type_id: crate::ir::UnionTypeId,
    tag: u64,
    payload: UnionPayloadSnapshot,
    payload_layout: crate::union_payload::UnionPayloadLayout,
}

/// Tetos do domínio interno de descritores de união.
///
/// Espelha `UnionBudgetLimits` do runtime nativo, inclusive na forma: os
/// limites são um **parâmetro do estado**, nunca uma leitura de ambiente. O
/// runtime de produção usa exclusivamente [`UNION_BUDGET_LIMITS`]; os testes do
/// próprio módulo constroem estados com limites reduzidos pelo mesmo campo, e
/// debug e release se comportam igual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnionBudgetLimits {
    max_descriptors: u64,
    max_payload_bytes: u64,
    max_metadata_bytes: u64,
}

/// Limites canônicos do domínio interno de descritores.
const UNION_BUDGET_LIMITS: UnionBudgetLimits = UnionBudgetLimits {
    max_descriptors: crate::union_payload::MAX_UNION_DESCRIPTORS,
    max_payload_bytes: crate::union_payload::MAX_UNION_TOTAL_PAYLOAD_BYTES,
    max_metadata_bytes: crate::union_payload::MAX_UNION_METADATA_BYTES,
};

/// Orçamento de recursos equivalente ao do runtime nativo. Sem isto, um laço
/// que injeta uniões cresceria sem limite no interpretador enquanto o nativo
/// falharia — quebra de paridade.
///
/// Este é o domínio **interno** de uniões: nada aqui consome a cota vitalícia
/// de identidades públicas, que pertence exclusivamente a `alocar`.
struct UnionRuntimeState {
    next_handle: usize,
    descriptors: HashMap<usize, UnionRuntimeDescriptor>,
    total_payload_bytes: u64,
    metadata_bytes: u64,
    limits: UnionBudgetLimits,
}

impl Default for UnionRuntimeState {
    fn default() -> Self {
        Self {
            next_handle: 0x7000_0000,
            descriptors: HashMap::new(),
            total_payload_bytes: 0,
            metadata_bytes: 0,
            limits: UNION_BUDGET_LIMITS,
        }
    }
}

impl UnionRuntimeState {
    /// Contabiliza um descritor novo contra os três orçamentos, com operações
    /// checked. Nenhum deles depende do profile de compilação.
    fn charge(&mut self, payload_size: u64) -> Result<(), PinkerError> {
        let descriptors = u64::try_from(self.descriptors.len())
            .map_err(|_| runtime_err("contagem de descritores de união excede u64"))?;
        if descriptors >= self.limits.max_descriptors {
            return Err(runtime_err(
                "E-RUNTIME-UNION-DESCRIPTOR-BUDGET: orçamento de descritores de união esgotado",
            ));
        }
        let total = self
            .total_payload_bytes
            .checked_add(payload_size)
            .ok_or_else(|| runtime_err("overflow no orçamento de bytes de união"))?;
        if total > self.limits.max_payload_bytes {
            return Err(runtime_err(
                "E-RUNTIME-UNION-PAYLOAD-BUDGET: orçamento de bytes de payload de união esgotado",
            ));
        }
        let metadata = self
            .metadata_bytes
            .checked_add(crate::union_payload::UNION_DESCRIPTOR_METADATA_BYTES)
            .ok_or_else(|| runtime_err("overflow no orçamento de metadata de união"))?;
        if metadata > self.limits.max_metadata_bytes {
            return Err(runtime_err(
                "E-RUNTIME-UNION-METADATA-BUDGET: orçamento de metadata de união esgotado",
            ));
        }
        self.total_payload_bytes = total;
        self.metadata_bytes = metadata;
        Ok(())
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
    // Fase 243/D3: contador de endereços simulados para a alocação possuída
    // pelo descritor dinâmico. O ambiente começa 16 bytes depois do descritor,
    // igual ao bloco contíguo do runtime nativo. A base fica bem acima de
    // qualquer endereço estático (`build_memory` começa em 1).
    next_allocation_addr: usize,
}

impl CallableState {
    fn new() -> Self {
        CallableState {
            table: HashMap::new(),
            next_handle: 1,
            static_by_name: HashMap::new(),
            next_allocation_addr: 0x1000_0000,
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
    fn create_closure_instance(
        &mut self,
        function_name: &str,
        captured: Vec<RuntimeValue>,
        memory: &mut HashMap<usize, RuntimeValue>,
    ) -> Result<u64, PinkerError> {
        const DESCRIPTOR_BYTES: usize = 16;

        let environment_bytes = captured
            .len()
            .checked_mul(8)
            .ok_or_else(|| runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: overflow no ambiente"))?;
        let allocation_bytes = DESCRIPTOR_BYTES
            .checked_add(environment_bytes)
            .ok_or_else(|| runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: overflow no layout"))?;
        let allocation_addr = self.next_allocation_addr;
        let next_allocation_addr =
            allocation_addr
                .checked_add(allocation_bytes)
                .ok_or_else(|| {
                    runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: espaço de endereços esgotado")
                })?;
        let handle = self.next_handle;
        let next_handle = handle.checked_add(1).ok_or_else(|| {
            runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: espaço de handles esgotado")
        })?;

        self.table.try_reserve(1).map_err(|_| {
            runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: descritor não pôde ser reservado")
        })?;
        memory.try_reserve(captured.len()).map_err(|_| {
            runtime_err("E-RUNTIME-CALLABLE-ALLOCATION: ambiente não pôde ser reservado")
        })?;

        let env_addr = (!captured.is_empty()).then_some(allocation_addr + DESCRIPTOR_BYTES);
        if let Some(base) = env_addr {
            for (index, value) in captured.into_iter().enumerate() {
                memory.insert(base + index * 8, value);
            }
        }
        self.table.insert(
            handle,
            CallableDescriptor {
                function_name: function_name.to_string(),
                env_addr,
            },
        );
        self.next_handle = next_handle;
        self.next_allocation_addr = next_allocation_addr;
        Ok(handle)
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
    maps: HashMap<u64, RuntimeGenericMap>,
    map_iters: HashMap<u64, RuntimeGenericMapIter>,
    // Adapter legado mantido para consumidores históricos (inclusive JSON).
    // O dispatch anteposto possui a semântica e espelha/importa estes campos;
    // handlers antigos abaixo permanecem inalcançáveis pelos entrypoints públicos.
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
    saidas_processo: crate::saida_processo::TabelaSaidas,
    valores_json: crate::valor_json::TabelaJson,
}

fn novo_runtime_map_state() -> RuntimeMapState {
    RuntimeMapState {
        maps_verso_bombom: HashMap::new(),
        maps_verso_verso: HashMap::new(),
        maps_bombom_bombom: HashMap::new(),
        maps_bombom_verso: HashMap::new(),
        maps: HashMap::new(),
        next_map_handle: 1,
        map_iters_verso_bombom: HashMap::new(),
        map_iters_verso_verso: HashMap::new(),
        map_iters_bombom_bombom: HashMap::new(),
        map_iters_bombom_verso: HashMap::new(),
        map_iters: HashMap::new(),
        next_map_iter_handle: 1,
        enum_values: HashMap::new(),
        next_enum_handle: 1,
        saidas_processo: crate::saida_processo::TabelaSaidas::nova(),
        valores_json: crate::valor_json::TabelaJson::nova(),
    }
}

#[derive(Clone, PartialEq, Eq)]
enum RuntimeMapKey {
    Bombom(u64),
    Verso(String),
}

struct RuntimeGenericMap {
    key_is_verso: bool,
    entries: Vec<(RuntimeMapKey, RuntimeValue)>,
}

struct RuntimeGenericMapIter {
    keys_snapshot: Vec<RuntimeValue>,
    next_index: usize,
}

enum RuntimeEnumPayload {
    Int(u64),
    Str(String),
    /// D1: handle opaco de uma palavra guardado numa variante.
    ///
    /// A cópia é **rasa por contrato**: o que entra na variante é o handle, e
    /// nada é clonado do conteúdo apontado. O backend nativo guarda exatamente
    /// a mesma palavra; aqui a categoria acompanha o handle apenas porque o
    /// interpretador tipa `RuntimeValue` e precisa devolver a lista com a
    /// mesma categoria com que ela entrou.
    ListBombom(u64),
    ListVerso(u64),
    SaidaProcesso(u64),
    ValorJson(u64),
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

fn generic_map_key(value: &RuntimeValue, key_is_verso: bool) -> Result<RuntimeMapKey, PinkerError> {
    if key_is_verso {
        let RuntimeValue::Str(value) = value else {
            return Err(runtime_err("mapa genérico exige chave 'verso'"));
        };
        Ok(RuntimeMapKey::Verso(value.clone()))
    } else {
        let RuntimeValue::Int(value) = value else {
            return Err(runtime_err("mapa genérico exige chave 'bombom'"));
        };
        Ok(RuntimeMapKey::Bombom(*value))
    }
}

fn map_runtime_handle(value: &RuntimeValue) -> Option<u64> {
    match value {
        RuntimeValue::Map(handle)
        | RuntimeValue::MapVersoBombom(handle)
        | RuntimeValue::MapVersoVerso(handle)
        | RuntimeValue::MapBombomBombom(handle)
        | RuntimeValue::MapBombomVerso(handle) => Some(*handle),
        _ => None,
    }
}

fn ensure_generic_map_from_legacy(
    state: &mut RuntimeMapState,
    value: &RuntimeValue,
) -> Result<u64, PinkerError> {
    let handle = map_runtime_handle(value).ok_or_else(|| runtime_err("valor não é mapa"))?;
    if state.maps.contains_key(&handle) {
        return Ok(handle);
    }
    let (key_is_verso, entries) = match value {
        RuntimeValue::MapVersoBombom(_) => (
            true,
            state
                .maps_verso_bombom
                .get(&handle)
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| {
                            (RuntimeMapKey::Verso(key.clone()), RuntimeValue::Int(*value))
                        })
                        .collect()
                })
                .ok_or_else(|| runtime_err("handle de mapa<verso,bombom> inválido"))?,
        ),
        RuntimeValue::MapVersoVerso(_) => (
            true,
            state
                .maps_verso_verso
                .get(&handle)
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| {
                            (
                                RuntimeMapKey::Verso(key.clone()),
                                RuntimeValue::Str(value.clone()),
                            )
                        })
                        .collect()
                })
                .ok_or_else(|| runtime_err("handle de mapa<verso,verso> inválido"))?,
        ),
        RuntimeValue::MapBombomBombom(_) => (
            false,
            state
                .maps_bombom_bombom
                .get(&handle)
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| {
                            (RuntimeMapKey::Bombom(*key), RuntimeValue::Int(*value))
                        })
                        .collect()
                })
                .ok_or_else(|| runtime_err("handle de mapa<bombom,bombom> inválido"))?,
        ),
        RuntimeValue::MapBombomVerso(_) => (
            false,
            state
                .maps_bombom_verso
                .get(&handle)
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| {
                            (
                                RuntimeMapKey::Bombom(*key),
                                RuntimeValue::Str(value.clone()),
                            )
                        })
                        .collect()
                })
                .ok_or_else(|| runtime_err("handle de mapa<bombom,verso> inválido"))?,
        ),
        RuntimeValue::Map(_) => return Err(runtime_err("handle de mapa genérico inválido")),
        _ => return Err(runtime_err("valor não é mapa")),
    };
    state.maps.insert(
        handle,
        RuntimeGenericMap {
            key_is_verso,
            entries,
        },
    );
    Ok(handle)
}

fn mirror_generic_map_to_legacy(
    state: &mut RuntimeMapState,
    value: &RuntimeValue,
) -> Result<(), PinkerError> {
    let Some(handle) = map_runtime_handle(value) else {
        return Ok(());
    };
    let Some(map) = state.maps.get(&handle) else {
        return Ok(());
    };
    match value {
        RuntimeValue::MapVersoBombom(_) => {
            let mut legacy = HashMap::new();
            for (key, value) in &map.entries {
                let (RuntimeMapKey::Verso(key), RuntimeValue::Int(value)) = (key, value) else {
                    return Err(runtime_err(
                        "representação legado verso/bombom incompatível",
                    ));
                };
                legacy.insert(key.clone(), *value);
            }
            state.maps_verso_bombom.insert(handle, legacy);
        }
        RuntimeValue::MapVersoVerso(_) => {
            let mut legacy = HashMap::new();
            for (key, value) in &map.entries {
                let (RuntimeMapKey::Verso(key), RuntimeValue::Str(value)) = (key, value) else {
                    return Err(runtime_err("representação legado verso/verso incompatível"));
                };
                legacy.insert(key.clone(), value.clone());
            }
            state.maps_verso_verso.insert(handle, legacy);
        }
        RuntimeValue::MapBombomBombom(_) => {
            let mut legacy = HashMap::new();
            for (key, value) in &map.entries {
                let (RuntimeMapKey::Bombom(key), RuntimeValue::Int(value)) = (key, value) else {
                    return Err(runtime_err(
                        "representação legado bombom/bombom incompatível",
                    ));
                };
                legacy.insert(*key, *value);
            }
            state.maps_bombom_bombom.insert(handle, legacy);
        }
        RuntimeValue::MapBombomVerso(_) => {
            let mut legacy = HashMap::new();
            for (key, value) in &map.entries {
                let (RuntimeMapKey::Bombom(key), RuntimeValue::Str(value)) = (key, value) else {
                    return Err(runtime_err(
                        "representação legado bombom/verso incompatível",
                    ));
                };
                legacy.insert(*key, value.clone());
            }
            state.maps_bombom_verso.insert(handle, legacy);
        }
        RuntimeValue::Map(_) => {}
        _ => return Err(runtime_err("valor não é mapa")),
    }
    Ok(())
}

fn try_call_map_intrinsic_authority(
    callee: &str,
    args: &[RuntimeValue],
    state: &mut RuntimeMapState,
) -> Option<Result<IntrinsicCall, PinkerError>> {
    let create = match callee {
        "__pinker_internal_mapa_criar_chave_bombom" => Some((false, 0_u8)),
        "__pinker_internal_mapa_criar_chave_verso" => Some((true, 0)),
        "mapa_verso_bombom_criar" => Some((true, 1)),
        "mapa_verso_verso_criar" => Some((true, 2)),
        "mapa_bombom_bombom_criar" => Some((false, 3)),
        "mapa_bombom_verso_criar" => Some((false, 4)),
        _ => None,
    };
    if let Some((key_is_verso, wrapper)) = create {
        return Some((|| {
            if !args.is_empty() {
                return Err(runtime_err("mapa_criar exige 0 argumentos"));
            }
            let handle = state.next_map_handle;
            state.next_map_handle = state.next_map_handle.saturating_add(1);
            state.maps.insert(
                handle,
                RuntimeGenericMap {
                    key_is_verso,
                    entries: Vec::new(),
                },
            );
            match wrapper {
                1 => {
                    state.maps_verso_bombom.insert(handle, HashMap::new());
                }
                2 => {
                    state.maps_verso_verso.insert(handle, HashMap::new());
                }
                3 => {
                    state.maps_bombom_bombom.insert(handle, HashMap::new());
                }
                4 => {
                    state.maps_bombom_verso.insert(handle, HashMap::new());
                }
                _ => {}
            }
            let value = match wrapper {
                0 => RuntimeValue::Map(handle),
                1 => RuntimeValue::MapVersoBombom(handle),
                2 => RuntimeValue::MapVersoVerso(handle),
                3 => RuntimeValue::MapBombomBombom(handle),
                4 => RuntimeValue::MapBombomVerso(handle),
                _ => unreachable!("wrapper histórico de mapa validado"),
            };
            Ok(IntrinsicCall::Done(Some(value)))
        })());
    }

    let define = matches!(
        callee,
        "__pinker_internal_mapa_definir"
            | "mapa_verso_bombom_definir"
            | "mapa_verso_verso_definir"
            | "mapa_bombom_bombom_definir"
            | "mapa_bombom_verso_definir"
    );
    if define {
        return Some((|| {
            if args.len() != 3 {
                return Err(runtime_err("mapa_definir exige 3 argumentos"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let key_is_verso = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?
                .key_is_verso;
            let key = generic_map_key(&args[1], key_is_verso)?;
            let map = state
                .maps
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            if let Some((_, current)) = map.entries.iter_mut().find(|(stored, _)| *stored == key) {
                *current = args[2].clone();
            } else {
                map.entries.push((key, args[2].clone()));
            }
            mirror_generic_map_to_legacy(state, &args[0])?;
            Ok(IntrinsicCall::Done(None))
        })());
    }

    let obtain = matches!(
        callee,
        "__pinker_internal_mapa_obter"
            | "mapa_verso_bombom_obter"
            | "mapa_verso_verso_obter"
            | "mapa_bombom_bombom_obter"
            | "mapa_bombom_verso_obter"
    );
    if obtain {
        return Some((|| {
            if args.len() != 2 {
                return Err(runtime_err("mapa_obter exige 2 argumentos"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let map = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            let key = generic_map_key(&args[1], map.key_is_verso)?;
            let value = map
                .entries
                .iter()
                .find(|(stored, _)| *stored == key)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| {
                    if callee.starts_with("mapa_") {
                        runtime_err(&format!("chave ausente em '{}'", callee))
                    } else {
                        runtime_err("chave ausente em leitura de mapa")
                    }
                })?;
            Ok(IntrinsicCall::Done(Some(value)))
        })());
    }

    let has = matches!(
        callee,
        "__pinker_internal_mapa_tem"
            | "mapa_verso_bombom_tem"
            | "mapa_verso_verso_tem"
            | "mapa_bombom_bombom_tem"
            | "mapa_bombom_verso_tem"
    );
    if has {
        return Some((|| {
            if args.len() != 2 {
                return Err(runtime_err("mapa_tem exige 2 argumentos"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let map = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            let key = generic_map_key(&args[1], map.key_is_verso)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                map.entries.iter().any(|(stored, _)| *stored == key),
            ))))
        })());
    }

    let size = matches!(
        callee,
        "__pinker_internal_mapa_tamanho"
            | "mapa_verso_bombom_tamanho"
            | "mapa_verso_verso_tamanho"
            | "mapa_bombom_bombom_tamanho"
            | "mapa_bombom_verso_tamanho"
    );
    if size {
        return Some((|| {
            if args.len() != 1 {
                return Err(runtime_err("mapa_tamanho exige 1 argumento"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let map = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                map.entries.len() as u64,
            ))))
        })());
    }

    let remove = matches!(
        callee,
        "__pinker_internal_mapa_remover"
            | "mapa_verso_bombom_remover"
            | "mapa_verso_verso_remover"
            | "mapa_bombom_bombom_remover"
            | "mapa_bombom_verso_remover"
    );
    if remove {
        return Some((|| {
            if args.len() != 2 {
                return Err(runtime_err("mapa_remover exige 2 argumentos"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let key_is_verso = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?
                .key_is_verso;
            let key = generic_map_key(&args[1], key_is_verso)?;
            let map = state
                .maps
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            if let Some(index) = map.entries.iter().position(|(stored, _)| *stored == key) {
                map.entries.remove(index);
            }
            mirror_generic_map_to_legacy(state, &args[0])?;
            Ok(IntrinsicCall::Done(None))
        })());
    }

    let iterator_create = matches!(
        callee,
        "__pinker_internal_mapa_iterador_criar"
            | "__pinker_internal_mapa_verso_bombom_iterador_criar"
            | "__pinker_internal_mapa_verso_verso_iterador_criar"
            | "__pinker_internal_mapa_bombom_bombom_iterador_criar"
            | "__pinker_internal_mapa_bombom_verso_iterador_criar"
    );
    if iterator_create {
        return Some((|| {
            if args.len() != 1 {
                return Err(runtime_err("iterador de mapa exige 1 argumento"));
            }
            let handle = ensure_generic_map_from_legacy(state, &args[0])?;
            let map = state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa inválido"))?;
            let keys_snapshot = map
                .entries
                .iter()
                .map(|(key, _)| match key {
                    RuntimeMapKey::Bombom(value) => RuntimeValue::Int(*value),
                    RuntimeMapKey::Verso(value) => RuntimeValue::Str(value.clone()),
                })
                .collect();
            let cursor = state.next_map_iter_handle;
            state.next_map_iter_handle = state.next_map_iter_handle.saturating_add(1);
            state.map_iters.insert(
                cursor,
                RuntimeGenericMapIter {
                    keys_snapshot,
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(cursor))))
        })());
    }

    let iterator_next = matches!(
        callee,
        "__pinker_internal_mapa_iterador_proxima_chave_bombom"
            | "__pinker_internal_mapa_iterador_proxima_chave_verso"
            | "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave"
            | "__pinker_internal_mapa_verso_verso_iterador_proxima_chave"
            | "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave"
            | "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave"
    );
    if iterator_next {
        return Some((|| {
            let [RuntimeValue::Int(cursor)] = args else {
                return Err(runtime_err("próxima chave exige cursor de mapa"));
            };
            let iterator = state
                .map_iters
                .get_mut(cursor)
                .ok_or_else(|| runtime_err("cursor de mapa inválido"))?;
            let value = iterator
                .keys_snapshot
                .get(iterator.next_index)
                .cloned()
                .ok_or_else(|| runtime_err("cursor de mapa esgotado"))?;
            iterator.next_index = iterator.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(value)))
        })());
    }

    None
}

fn generic_map_key_value(key: &RuntimeMapKey) -> RuntimeValue {
    match key {
        RuntimeMapKey::Bombom(value) => RuntimeValue::Int(*value),
        RuntimeMapKey::Verso(value) => RuntimeValue::Str(value.clone()),
    }
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

#[derive(Debug, Clone, Copy)]
enum CallResultTarget {
    DirectValue,
    DirectVoid,
    IndirectValue,
    Raw { has_return: bool },
    Trait { has_return: bool },
}

struct PendingCall {
    fn_name: String,
    args: Vec<RuntimeValue>,
    result_target: CallResultTarget,
}

enum InstrControl {
    Continue,
    Call(PendingCall),
}

struct ExecutionFrame {
    function_index: usize,
    slots: HashMap<String, RuntimeValue>,
    stack: Vec<RuntimeValue>,
    labels: HashMap<String, usize>,
    current_label: String,
    next_instr: usize,
    result_target: Option<CallResultTarget>,
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
    Map(u64),
    /// Handle nominal de snapshot de processo. Nunca é confundido com lista.
    SaidaProcesso(u64),
    /// Parte E1: handle nominal da raiz de uma árvore JSON.
    ///
    /// Categoria distinta de `SaidaProcesso` de propósito: as duas são handles
    /// de uma palavra, mas confundi-las faria um acessor de uma família aceitar
    /// valor da outra.
    ValorJson(u64),
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
    let mut public_memory_state = PublicMemoryState::default();
    UNION_RUNTIME_STATE.with(|state| *state.borrow_mut() = UnionRuntimeState::default());
    run_program_com_estado(program, cli_args, &mut public_memory_state)
}

/// Execução hospedada sobre um estado de memória pública fornecido pelo chamador.
///
/// Existe para que a contabilidade seja **observável e configurável por dentro**:
/// os testes do próprio módulo constroem um [`PublicMemoryState`] com limites
/// reduzidos, executam o programa e inspecionam as cotas consumidas. Não há
/// variável de ambiente nem opção de linha de comando: `run_program_with_args`
/// sempre usa os limites canônicos, e esta função não é exportada da crate.
fn run_program_com_estado(
    program: &MachineProgram,
    cli_args: &[String],
    public_memory_state: &mut PublicMemoryState,
) -> Result<RunOutcome, PinkerError> {
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
    let mut map_state = novo_runtime_map_state();
    let mut random_state = RuntimeRandomState {
        generators: HashMap::new(),
        next_generator_handle: 1,
    };
    let mut callable_state = CallableState::new();
    let mut trait_object_state = TraitObjectState::new();
    let mut call_stack = Vec::new();
    let return_value = call_function(
        "principal",
        vec![],
        program,
        &globals,
        &mut memory,
        public_memory_state,
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

fn prepare_execution_frame(
    fn_name: &str,
    args: Vec<RuntimeValue>,
    result_target: Option<CallResultTarget>,
    program: &MachineProgram,
) -> Result<ExecutionFrame, PinkerError> {
    let function = find_function(fn_name, program)?;
    if function.params.len() != args.len() {
        return Err(runtime_err(&format!(
            "[{}] chamada com aridade inválida",
            fn_name
        )));
    }

    let function_index = program
        .functions
        .iter()
        .position(|candidate| candidate.name == fn_name)
        .expect("find_function confirmou a existência da função");
    let labels = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label.clone(), index))
        .collect();

    let mut slots = HashMap::new();
    for (slot, value) in function.params.iter().cloned().zip(args.into_iter()) {
        let coerced = if let Some(ty) = function.slot_types.get(&slot) {
            coerce_runtime_value_to_type(value, *ty)?
        } else {
            value
        };
        slots.insert(slot, coerced);
    }

    Ok(ExecutionFrame {
        function_index,
        slots,
        stack: Vec::new(),
        labels,
        current_label: "entry".to_string(),
        next_instr: 0,
        result_target,
    })
}

fn accept_call_result(
    target: CallResultTarget,
    result: Option<RuntimeValue>,
    stack: &mut Vec<RuntimeValue>,
) -> Result<(), PinkerError> {
    match (target, result) {
        (CallResultTarget::DirectValue, Some(value))
        | (CallResultTarget::IndirectValue, Some(value))
        | (CallResultTarget::Raw { has_return: true }, Some(value))
        | (CallResultTarget::Trait { has_return: true }, Some(value)) => stack.push(value),
        (CallResultTarget::DirectValue, None) => {
            return Err(runtime_err("call exige função com retorno"));
        }
        (CallResultTarget::DirectVoid, Some(_)) => {
            return Err(runtime_err("call_void exige função sem retorno"));
        }
        (CallResultTarget::DirectVoid, None) => {}
        (CallResultTarget::IndirectValue, None) => {
            return Err(runtime_err(
                "call_indirect exige callable com retorno (tipo função público nunca é nulo)",
            ));
        }
        (CallResultTarget::Raw { has_return: true }, None) => {
            return Err(runtime_err("call_raw esperava retorno com valor"));
        }
        (CallResultTarget::Raw { has_return: false }, None) => {}
        (CallResultTarget::Raw { has_return: false }, Some(_)) => {
            return Err(runtime_err("call_raw nulo recebeu retorno com valor"));
        }
        (CallResultTarget::Trait { has_return: false }, None) => {}
        (CallResultTarget::Trait { has_return: false }, Some(_)) => {
            return Err(runtime_err("trait_call nulo recebeu retorno inesperado"));
        }
        (CallResultTarget::Trait { has_return: true }, None) => {
            return Err(runtime_err("trait_call com retorno recebeu função nulo"));
        }
    }
    Ok(())
}

// Executa funções numa pilha explícita de frames. Chamadas Pinker suspendem o
// frame atual e empilham outro; somente intrínsecas hospedadas executam inline.
// O call_stack permanece separado para preservar o stack trace observável.
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
    let call_stack_base = call_stack.len();
    call_stack.push(RuntimeFrame {
        fn_name: fn_name.to_string(),
        block_label: None,
        current_instr: None,
        future_span: None,
    });

    let result = (|| {
        let initial = prepare_execution_frame(fn_name, args, None, program)?;
        let mut execution_stack = vec![initial];

        loop {
            if io_state.exit_status.is_some() {
                let completed = execution_stack
                    .pop()
                    .expect("a pilha de execução contém ao menos o frame raiz");
                let _ = call_stack.pop();
                let Some(parent) = execution_stack.last_mut() else {
                    return Ok(None);
                };
                accept_call_result(
                    completed
                        .result_target
                        .expect("somente o frame raiz não possui destino de retorno"),
                    None,
                    &mut parent.stack,
                )?;
                set_current_instr(call_stack, None);
                continue;
            }

            let frame_index = execution_stack.len() - 1;
            let function_index = execution_stack[frame_index].function_index;
            let function = &program.functions[function_index];
            let current_label = execution_stack[frame_index].current_label.clone();
            let Some(&block_idx) = execution_stack[frame_index].labels.get(&current_label) else {
                return Err(runtime_err(&format!(
                    "[{}] label de execução inexistente: {}",
                    function.name, current_label
                )));
            };
            let block = &function.blocks[block_idx];
            if let Some(frame) = call_stack.last_mut() {
                frame.block_label = Some(block.label.clone());
            }

            if execution_stack[frame_index].next_instr < block.code.len() {
                let instr_index = execution_stack[frame_index].next_instr;
                let instr = &block.code[instr_index];
                set_current_instr(call_stack, Some(machine_instr_name(instr)));
                let control = {
                    let frame = &mut execution_stack[frame_index];
                    exec_instr(
                        instr,
                        &mut frame.slots,
                        &mut frame.stack,
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
                    )?
                };

                execution_stack[frame_index].next_instr += 1;
                match control {
                    InstrControl::Continue => set_current_instr(call_stack, None),
                    InstrControl::Call(pending) => {
                        call_stack.push(RuntimeFrame {
                            fn_name: pending.fn_name.clone(),
                            block_label: None,
                            current_instr: None,
                            future_span: None,
                        });
                        let next = prepare_execution_frame(
                            &pending.fn_name,
                            pending.args,
                            Some(pending.result_target),
                            program,
                        )?;
                        execution_stack.push(next);
                    }
                }
                continue;
            }

            let completed_result = match block.terminator.clone() {
                MachineTerminator::Jmp(target) => {
                    execution_stack[frame_index].current_label = target;
                    execution_stack[frame_index].next_instr = 0;
                    None
                }
                MachineTerminator::BrTrue {
                    then_label,
                    else_label,
                } => {
                    let cond = pop_bool(
                        &mut execution_stack[frame_index].stack,
                        "br_true requer bool no topo",
                    )?;
                    execution_stack[frame_index].current_label =
                        if cond { then_label } else { else_label };
                    execution_stack[frame_index].next_instr = 0;
                    None
                }
                MachineTerminator::Ret => {
                    if execution_stack[frame_index].stack.len() != 1 {
                        return Err(runtime_err(&format!(
                            "[{}] ret inválido: pilha deve ter 1 valor",
                            function.name
                        )));
                    }
                    let value = execution_stack[frame_index]
                        .stack
                        .pop()
                        .expect("len checked");
                    Some(Some(coerce_runtime_value_to_type(
                        value,
                        function.ret_type,
                    )?))
                }
                MachineTerminator::RetVoid => {
                    if !execution_stack[frame_index].stack.is_empty() {
                        return Err(runtime_err(&format!(
                            "[{}] ret_void inválido: pilha deve estar vazia",
                            function.name
                        )));
                    }
                    Some(None)
                }
            };

            let Some(completed_result) = completed_result else {
                continue;
            };
            let completed = execution_stack
                .pop()
                .expect("o frame concluído está no topo da pilha");
            let _ = call_stack.pop();
            let Some(parent) = execution_stack.last_mut() else {
                return Ok(completed_result);
            };
            accept_call_result(
                completed
                    .result_target
                    .expect("somente o frame raiz não possui destino de retorno"),
                completed_result,
                &mut parent.stack,
            )?;
            set_current_instr(call_stack, None);
        }
    })();

    let result = result.map_err(|err| attach_runtime_trace(err, call_stack));
    call_stack.truncate(call_stack_base);
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
    call_stack: &mut [RuntimeFrame],
) -> Result<InstrControl, PinkerError> {
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
                RuntimeValue::MapBombomVerso(_) | RuntimeValue::Map(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
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
                RuntimeValue::MapBombomVerso(_) | RuntimeValue::Map(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
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
            // HR3: um agregado — array fixo **ou** `ninho` — é representado
            // pelo endereço da sua representação completa. Abrir `*ptr` não lê
            // uma palavra: produz o mesmo endereço, que é o que a injeção de
            // união entrega para a cópia integral do snapshot. Tratar só array
            // fixo aqui deixava `ninho` sem caminho de execução.
            if matches!(
                ty,
                crate::ir::TypeIR::FixedArray { .. } | crate::ir::TypeIR::Struct
            ) {
                stack.push(RuntimeValue::Ptr(addr));
                return Ok(InstrControl::Continue);
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
                public_memory_store_bytes(&mut public_memory_state.payload, addr, *ty, coerced)?;
                return Ok(InstrControl::Continue);
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
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_layout,
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
            if member.ty != *payload_type || member.payload_layout != *payload_layout {
                return Err(runtime_err("layout de união divergente no runtime"));
            }
            if !payload_layout.is_well_formed() {
                return Err(runtime_err("layout de união mal formado no runtime"));
            }
            // O interpretador confere a identidade do membro em vez de aceitar a
            // tag isolada: uma tag que não corresponda à identidade decidida no
            // lowering é falha de compilação observada em execução, não um
            // resultado a ser produzido silenciosamente.
            if member.resolved_type_id != *resolved_member_type_id
                || member.canonical_member_key != *canonical_member_key
            {
                return Err(runtime_err(
                    "identidade de membro de união divergente no runtime",
                ));
            }
            // A cópia acontece **antes** de qualquer registro: o snapshot é
            // materializado a partir da origem e o descritor passa a ser
            // independente dela.
            let snapshot =
                union_snapshot_from_source(payload, *payload_layout, public_memory_state)?;
            let handle = UNION_RUNTIME_STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.charge(payload_layout.size)?;
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
                        payload: snapshot,
                        payload_layout: *payload_layout,
                    },
                );
                Ok::<usize, PinkerError>(handle)
            })?;
            stack.push(RuntimeValue::Ptr(handle));
        }
        // Execução direta das operações internas tipadas de união: `union_tag` valida o `UnionTypeId` do descritor e devolve a tag corrente; `union_extract` valida união, tag, chave canônica e layout contra a tabela internada antes de devolver o payload. Não há despacho por nome `__pinker_internal_*` — descritor inválido produz diagnóstico estruturado.
        MachineInstr::UnionTag { union_type_id } => {
            let value = pop(stack, "union_tag exige valor de união no topo")?;
            let RuntimeValue::Ptr(handle) = value else {
                return Err(runtime_err("union_tag exige handle de união"));
            };
            crate::ir::validate_union_reference(&program.union_types, *union_type_id)
                .map_err(|message| runtime_err(&message))?;
            let tag = UNION_RUNTIME_STATE.with(|state| {
                let state = state.borrow();
                let descriptor = state
                    .descriptors
                    .get(&handle)
                    .ok_or_else(|| runtime_err("handle de união inválido em union_tag"))?;
                if descriptor.union_type_id != *union_type_id {
                    return Err(runtime_err("descritor de união de outro tipo em union_tag"));
                }
                Ok(descriptor.tag)
            })?;
            stack.push(RuntimeValue::Int(tag));
        }
        MachineInstr::UnionExtract {
            union_type_id,
            tag,
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_layout,
        } => {
            let value = pop(stack, "union_extract exige valor de união no topo")?;
            let RuntimeValue::Ptr(handle) = value else {
                return Err(runtime_err("union_extract exige handle de união"));
            };
            crate::ir::validate_union_member_reference(
                &program.union_types,
                *union_type_id,
                *tag,
                canonical_member_key,
                *payload_type,
                *payload_layout,
            )
            .map_err(|message| runtime_err(&message))?;
            crate::ir::validate_union_member_identity(
                &program.union_types,
                *union_type_id,
                *tag,
                *resolved_member_type_id,
            )
            .map_err(|message| runtime_err(&message))?;
            let descriptor = UNION_RUNTIME_STATE.with(|state| {
                state
                    .borrow()
                    .descriptors
                    .get(&handle)
                    .cloned()
                    .ok_or_else(|| runtime_err("handle de união inválido em union_extract"))
            })?;
            if descriptor.union_type_id != *union_type_id {
                return Err(runtime_err(
                    "descritor de união de outro tipo em union_extract",
                ));
            }
            if descriptor.tag != *tag {
                return Err(runtime_err("tag divergente ao abrir payload de união"));
            }
            if descriptor.payload_layout != *payload_layout
                || !descriptor.payload_layout.is_well_formed()
            {
                return Err(runtime_err("layout inválido no descritor de união"));
            }
            // Storage novo a cada extração: o binding pode ser mudado sem tocar
            // no snapshot, e duas extrações da mesma união não compartilham
            // memória.
            let payload = union_snapshot_to_binding(
                &descriptor.payload,
                *payload_layout,
                *payload_type,
                public_memory_state,
            )?;
            stack.push(payload);
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
        MachineInstr::PointerOffset {
            element_size,
            element_align,
        } => {
            let offset = pop(stack, "pointer_offset exige deslocamento no topo")?;
            let pointer = pop(
                stack,
                "pointer_offset exige ponteiro abaixo do deslocamento",
            )?;
            let RuntimeValue::Ptr(source) = pointer else {
                return Err(runtime_err("pointer_offset exige operando seta<T>"));
            };
            if source == 0 {
                return Err(runtime_err(
                    "E-RUNTIME-POINTER-NULL-ARITHMETIC: aritmética sobre ponteiro nulo",
                ));
            }
            let RuntimeValue::Int(offset) = offset else {
                return Err(runtime_err(
                    "pointer_offset exige deslocamento bombom não negativo",
                ));
            };
            if *element_size == 0
                || *element_align == 0
                || !element_align.is_power_of_two()
                || *element_size % *element_align != 0
            {
                return Err(runtime_err(
                    "E-RUNTIME-POINTER-LAYOUT: layout de elemento inválido",
                ));
            }
            let byte_delta = offset.checked_mul(*element_size).ok_or_else(|| {
                runtime_err("E-RUNTIME-POINTER-OFFSET-OVERFLOW: overflow ao escalar deslocamento")
            })?;
            let byte_delta = usize::try_from(byte_delta).map_err(|_| {
                runtime_err("E-RUNTIME-POINTER-OFFSET-OVERFLOW: deslocamento excede a plataforma")
            })?;
            // A memória simulada histórica usa handles de slot (1, 2, 3...),
            // não endereços de byte. Para essa origem interna o offset é
            // aplicado em elementos; regiões públicas e endereços fabricados
            // continuam usando o delta em bytes, como o nativo.
            let physical_delta = if public_memory_region(public_memory_state, source).is_none()
                && memory.contains_key(&source)
            {
                usize::try_from(offset).map_err(|_| {
                    runtime_err(
                        "E-RUNTIME-POINTER-OFFSET-OVERFLOW: deslocamento excede a plataforma",
                    )
                })?
            } else {
                byte_delta
            };
            let derived = source.checked_add(physical_delta).ok_or_else(|| {
                runtime_err("E-RUNTIME-POINTER-ADDRESS-OVERFLOW: overflow ao derivar endereço")
            })?;
            let result = RuntimeValue::Ptr(derived);
            validar_derivacao_memoria_publica(public_memory_state, Some(source), &result)?;
            stack.push(result);
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
        MachineInstr::Call {
            callee,
            argc,
            identidade,
        } => {
            let args = pop_args(stack, *argc)?;
            // #532: só a IDENTIDADE abre a tabela de intrínsecas. Uma função do
            // usuário com grafia canônica chega aqui como `CalleeIdentity::User`
            // e vai direto para a chamada Pinker comum.
            let result = match try_call_intrinsic(
                *identidade,
                callee,
                &args,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => {
                    return Ok(InstrControl::Call(PendingCall {
                        fn_name: callee.clone(),
                        args,
                        result_target: CallResultTarget::DirectValue,
                    }));
                }
            };
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid {
            callee,
            argc,
            identidade,
        } => {
            let args = pop_args(stack, *argc)?;
            let result = match try_call_intrinsic(
                *identidade,
                callee,
                &args,
                public_memory_state,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => {
                    return Ok(InstrControl::Call(PendingCall {
                        fn_name: callee.clone(),
                        args,
                        result_target: CallResultTarget::DirectVoid,
                    }));
                }
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
            let handle = callable_state.create_closure_instance(function_name, captured, memory)?;
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
            return Ok(InstrControl::Call(PendingCall {
                fn_name: function_name,
                args: combined_args,
                result_target: CallResultTarget::IndirectValue,
            }));
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
            return Ok(InstrControl::Call(PendingCall {
                fn_name: function_name.to_string(),
                args,
                result_target: CallResultTarget::Raw {
                    has_return: *has_return,
                },
            }));
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

            return Ok(InstrControl::Call(PendingCall {
                fn_name: function_name,
                args: combined_args,
                result_target: CallResultTarget::Trait {
                    has_return: *ret_type != crate::ir::TypeIR::Nulo,
                },
            }));
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
                RuntimeValue::MapBombomVerso(_) | RuntimeValue::Map(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
                RuntimeValue::Callable(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => {
                    unreachable!("pop_numeric só retorna inteiro")
                }
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

    Ok(InstrControl::Continue)
}

// @pinker-nav:end interpreter.execucao.instrucoes-pilha

// @pinker-nav:start interpreter.falha-operacional.construcao
// @pinker-nav:domain erros
// @pinker-nav:layer interpreter
// @pinker-nav:summary Execução hospedada das superfícies falíveis da Parte B: `executar_superficie_falivel` despacha por `OperacaoFalivel` — nunca por nome literal — e obtém nome público e tipo do argumento da própria autoridade; `exigir_argumento_unico` mantém aridade e tipo como erro de programa; `novo_leque`/`resultado_ok_bombom`/`resultado_ok_verso`/`resultado_erro` produzem o valor pelo mesmo `enum_values` que qualquer leque do usuário, com `Ok` na tag 0 e `Erro` na tag 1.
/// Argumento único das superfícies falíveis, com o tipo exigido vindo da
/// autoridade em vez de ser reafirmado aqui.
///
/// Aridade e tipo errados são erro de programa, detectados antes daqui pela
/// semântica; esta checagem é a rede do interpretador e continua fatal — não
/// vira `Erro(...)`.
fn validar_argumentos_superficie(
    superficie: &SuperficieFalivel,
    args: &[RuntimeValue],
) -> Result<(), PinkerError> {
    if args.len() != superficie.aridade() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige {} argumento(s)",
            superficie.intrinseca,
            superficie.aridade()
        )));
    }
    for (index, (valor, esperado)) in args.iter().zip(superficie.argumentos.iter()).enumerate() {
        let valido = matches!(
            (esperado, valor),
            (
                crate::falha_operacional::CargaResultado::Bombom,
                RuntimeValue::Int(_)
            ) | (
                crate::falha_operacional::CargaResultado::Verso,
                RuntimeValue::Str(_)
            ) | (
                crate::falha_operacional::CargaResultado::ListaVerso,
                RuntimeValue::ListVerso(_)
            ) | (
                crate::falha_operacional::CargaResultado::MapaVersoVerso,
                RuntimeValue::MapVersoVerso(_),
            ) | (
                crate::falha_operacional::CargaResultado::SaidaProcesso,
                RuntimeValue::SaidaProcesso(_),
            ) | (
                crate::falha_operacional::CargaResultado::ValorJson,
                RuntimeValue::ValorJson(_),
            ) | (
                crate::falha_operacional::CargaResultado::Leque(_),
                RuntimeValue::Int(_)
            )
        );
        if !valido {
            return Err(runtime_err(&format!(
                "intrínseca '{}' exige argumento {} em {}",
                superficie.intrinseca,
                index + 1,
                esperado.nome_para_diagnostico()
            )));
        }
    }
    Ok(())
}

/// SHA-256 dos bytes exatos de um arquivo, em streaming.
///
/// Compartilha o núcleo com o runtime nativo (`pinker_sha256_contract`), então
/// o digest é idêntico nos dois backends por construção, e não por dois
/// caminhos que por acaso concordam.
///
/// Lê em blocos para que o custo de memória não acompanhe o tamanho do arquivo:
/// o acumulador guarda 8 palavras e um bloco parcial, nada mais. O `File` e o
/// buffer são estritamente locais — fechados por drop ao sair, no sucesso e no
/// erro —, logo nenhuma identidade pública de recurso é criada.
fn sha256_de_arquivo(caminho: &str) -> std::io::Result<String> {
    use std::io::Read;

    let mut arquivo = fs::File::open(caminho)?;
    let mut acumulador = pinker_sha256_contract::Sha256::novo();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let lidos = arquivo.read(&mut buffer)?;
        if lidos == 0 {
            break;
        }
        acumulador.atualizar(&buffer[..lidos]);
    }
    Ok(acumulador.finalizar_hex())
}

/// Executa uma superfície falível já resolvida pela autoridade.
///
/// O `match` é sobre [`OperacaoFalivel`], não sobre o nome público: acrescentar
/// uma superfície nova sem tratar sua operação vira erro de exaustividade em
/// compilação, e o nome público continua existindo em um único lugar.
fn executar_superficie_falivel(
    superficie: &SuperficieFalivel,
    args: &[RuntimeValue],
    map_state: &mut RuntimeMapState,
    list_state: &mut RuntimeListState,
) -> Result<IntrinsicCall, PinkerError> {
    validar_argumentos_superficie(superficie, args)?;
    let entrada = match args.first() {
        Some(RuntimeValue::Str(texto)) => texto.as_str(),
        _ => "",
    };
    match superficie.operacao {
        OperacaoFalivel::LerArquivoPorCaminho => match fs::read_to_string(entrada) {
            Ok(conteudo) => Ok(resultado_ok_verso(map_state, conteudo)),
            Err(err) => Ok(resultado_erro(
                map_state,
                format!("falha ao ler arquivo '{}': {}", entrada, err),
            )),
        },
        // Parte E2: hash dos bytes EXATOS do arquivo.
        //
        // Lê em streaming pelo mesmo núcleo compartilhado com o runtime nativo,
        // deliberadamente sem `read_to_string`: UTF-8 inválido é conteúdo
        // legítimo, e nenhum byte pode ser validado, normalizado ou substituído
        // no caminho. Diretório e ausência falham no próprio SO e atravessam
        // `Resultado` como valor.
        OperacaoFalivel::HashArquivo => match sha256_de_arquivo(entrada) {
            Ok(digest) => Ok(resultado_ok_verso(map_state, digest)),
            Err(err) => Ok(resultado_erro(
                map_state,
                format!("falha ao hashear arquivo '{}': {}", entrada, err),
            )),
        },
        OperacaoFalivel::ExecutarProcesso => {
            // Comando vazio permanece erro de uso, não falha ambiental.
            validar_comando_nao_vazio(superficie.intrinseca, entrada)?;
            let mut processo = comando_de_processo(entrada);
            match processo.status() {
                // Spawn bem-sucedido: o código de saída do filho é valor de
                // sucesso. Código não representável continua fatal — a
                // modelagem de status pertence à Parte D.
                Ok(status) => {
                    let codigo = exit_code_u64(superficie.intrinseca, status.code())?;
                    Ok(resultado_ok_bombom(map_state, codigo))
                }
                Err(err) => Ok(resultado_erro(
                    map_state,
                    format!("falha ao executar processo '{}': {}", entrada, err),
                )),
            }
        }
        OperacaoFalivel::ConverterVersoParaBombom => match entrada.trim().parse::<u64>() {
            Ok(valor) => Ok(resultado_ok_bombom(map_state, valor)),
            Err(_) => Ok(resultado_erro(
                map_state,
                format!("falha ao converter '{}' para bombom", entrada),
            )),
        },
        OperacaoFalivel::EnumerarDiretorio => match enumerar_diretorio(entrada) {
            Ok(nomes) => Ok(resultado_ok_lista_verso(map_state, list_state, nomes)),
            Err(causa) => Ok(resultado_erro(map_state, causa)),
        },
        OperacaoFalivel::ClassificarEntrada => match fs::symlink_metadata(entrada) {
            Ok(meta) => Ok(resultado_ok_bombom(
                map_state,
                crate::tipo_entrada::TipoEntrada::classificar(meta.file_type()).discriminante(),
            )),
            Err(err) => Ok(resultado_erro(
                map_state,
                format!("falha ao classificar entrada '{}': {}", entrada, err),
            )),
        },
        OperacaoFalivel::MedirEntrada => match fs::symlink_metadata(entrada) {
            Ok(meta) => Ok(resultado_ok_bombom(map_state, meta.len())),
            Err(err) => Ok(resultado_erro(
                map_state,
                format!("falha ao medir entrada '{}': {}", entrada, err),
            )),
        },
        OperacaoFalivel::InterpretarJson => {
            // Dado externo malformado é falha recuperável, não aborto: a
            // separação entre dado externo, erro estático de programa e
            // violação de invariante é a razão de a Parte B existir.
            match crate::valor_json::interpretar(entrada, &mut map_state.valores_json) {
                Ok(raiz) => Ok(resultado_ok_valor_json(map_state, raiz)),
                Err(causa) => Ok(resultado_erro(map_state, causa)),
            }
        }
        OperacaoFalivel::ExecutarProcessoEstruturado => {
            let [RuntimeValue::Str(programa), RuntimeValue::ListVerso(argumentos_handle), RuntimeValue::Str(entrada), RuntimeValue::Str(diretorio), RuntimeValue::MapVersoVerso(ambiente_handle), RuntimeValue::Int(limite_handle)] =
                args
            else {
                unreachable!("assinatura validada pela autoridade falível")
            };
            let argumentos = list_state
                .lists_verso
                .get(argumentos_handle)
                .cloned()
                .ok_or_else(|| {
                    runtime_err(&format!(
                        "handle lista<verso> inválido em '{}'",
                        crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO
                    ))
                })?;
            let ambiente = map_state
                .maps_verso_verso
                .get(ambiente_handle)
                .cloned()
                .ok_or_else(|| {
                    runtime_err(&format!(
                        "handle mapa<verso,verso> inválido em '{}'",
                        crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO
                    ))
                })?;
            let limite = limite_tempo_do_runtime(map_state, *limite_handle)?;
            let configuracao =
                crate::processo_estruturado_hospedado::ConfiguracaoProcessoEstruturado {
                    programa,
                    argumentos: &argumentos,
                    entrada,
                    diretorio,
                    ambiente: &ambiente,
                    limite,
                };
            match crate::processo_estruturado_hospedado::executar(&configuracao) {
                Ok(saida) => Ok(resultado_ok_saida_processo(map_state, saida)),
                Err(causa) => Ok(resultado_erro(map_state, causa)),
            }
        }
    }
}

fn limite_tempo_do_runtime(
    map_state: &RuntimeMapState,
    handle: u64,
) -> Result<crate::limite_tempo::LimiteTempo, PinkerError> {
    let (tag, cargas) = map_state.enum_values.get(&handle).ok_or_else(|| {
        runtime_err(&format!(
            "handle LimiteTempo inválido em '{}'",
            crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO
        ))
    })?;
    match (*tag, cargas.as_slice()) {
        (crate::limite_tempo::TAG_SEM_LIMITE, []) => {
            Ok(crate::limite_tempo::LimiteTempo::SemLimite)
        }
        (crate::limite_tempo::TAG_ATE, [RuntimeEnumPayload::Int(milisegundos)]) => {
            Ok(crate::limite_tempo::LimiteTempo::Ate(*milisegundos))
        }
        _ => Err(runtime_err(&format!(
            "valor LimiteTempo inválido em '{}'",
            crate::falha_operacional::EXECUTAR_PROCESSO_ESTRUTURADO
        ))),
    }
}

/// Enumeração determinística das entradas imediatas de um diretório.
///
/// Compartilha o contrato com o runtime nativo, não a implementação: as duas
/// pontas repetem os mesmos passos em linguagens diferentes e a paridade é
/// fixada por evidência.
///
/// A ordem devolvida por `read_dir` é do filesystem e não é contrato de
/// ninguém; a ordenação explícita ao final é o que torna o resultado
/// observável e repetível.
fn enumerar_diretorio(caminho: &str) -> Result<Vec<String>, String> {
    // O argumento é inspecionado sem seguir link: um symlink que aponta para
    // diretório é recusado, porque enumerá-lo seria segui-lo.
    let meta = fs::symlink_metadata(caminho)
        .map_err(|err| format!("falha ao listar diretório '{}': {}", caminho, err))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "falha ao listar diretório '{}': o caminho é um link simbólico e \
             não é seguido por padrão",
            caminho
        ));
    }
    if !meta.is_dir() {
        return Err(format!(
            "falha ao listar diretório '{}': o caminho não é um diretório",
            caminho
        ));
    }
    let entradas = fs::read_dir(caminho)
        .map_err(|err| format!("falha ao listar diretório '{}': {}", caminho, err))?;
    let mut nomes = Vec::new();
    for entrada in entradas {
        // Erro no meio da iteração é falha da operação inteira: devolver o que
        // deu certo até aqui entregaria uma listagem silenciosamente parcial.
        let entrada =
            entrada.map_err(|err| format!("falha ao listar diretório '{}': {}", caminho, err))?;
        let bruto = entrada.file_name();
        // `.` e `..` não são produzidos por `read_dir`; a exclusão é contrato e
        // não depende disso, então não há filtro a acrescentar aqui.
        match bruto.to_str() {
            Some(nome) => nomes.push(nome.to_string()),
            // Nome não representável como `verso`. Não é pulado, não é
            // substituído e não vira texto lossy: a operação inteira falha.
            None => {
                // O nome entra no diagnóstico pela forma escapada de `OsStr`
                // (`\xFF`), não por `to_string_lossy`: substituir os bytes por
                // U+FFFD colocaria uma versão lossy do nome dentro de um valor
                // observável, que é exatamente o que o contrato proíbe. A forma
                // escapada é fiel e não pode ser confundida com o nome real.
                return Err(format!(
                    "falha ao listar diretório '{}': a entrada {:?} não é \
                     representável como verso (UTF-8 inválido)",
                    caminho, bruto
                ));
            }
        }
    }
    nomes.sort_unstable();
    Ok(nomes)
}

/// Cria um valor de leque com a tag dada, pelo mesmo caminho de
/// `__pinker_internal_leque_criar_0`.
fn novo_leque(map_state: &mut RuntimeMapState, tag: u64) -> u64 {
    let handle = map_state.next_enum_handle;
    map_state.next_enum_handle = map_state.next_enum_handle.saturating_add(1);
    map_state.enum_values.insert(handle, (tag, Vec::new()));
    handle
}

/// `Resultado.Ok(valor)` com carga de uma palavra.
fn resultado_ok_bombom(map_state: &mut RuntimeMapState, valor: u64) -> IntrinsicCall {
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_OK);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::Int(valor));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}

/// `Resultado.Ok(texto)` com carga textual.
fn resultado_ok_verso(map_state: &mut RuntimeMapState, texto: String) -> IntrinsicCall {
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_OK);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::Str(texto));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}

/// `Resultado.Ok(SaidaProcesso)` só é materializado depois de execução,
/// captura, reap, status normal e UTF-8 estrito concluírem com sucesso.
fn resultado_ok_saida_processo(
    map_state: &mut RuntimeMapState,
    saida: crate::saida_processo::SaidaProcesso,
) -> IntrinsicCall {
    let snapshot = map_state.saidas_processo.inserir(saida);
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_OK);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::SaidaProcesso(snapshot));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}

/// `Resultado.Ok(ValorJson)` — Parte E1.
///
/// A árvore já está materializada na tabela quando o handle da raiz entra na
/// variante. A carga é o handle, como em todas as demais famílias por handle.
fn resultado_ok_valor_json(map_state: &mut RuntimeMapState, raiz: u64) -> IntrinsicCall {
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_OK);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::ValorJson(raiz));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}

/// `Resultado.Ok(lista)` com carga de coleção — Parte C.
///
/// A lista é criada pelo mesmo caminho de `lista_verso_criar` e entra na
/// variante como handle de uma palavra. A cópia continua rasa por contrato (D1):
/// o que a variante guarda é o handle.
fn resultado_ok_lista_verso(
    map_state: &mut RuntimeMapState,
    list_state: &mut RuntimeListState,
    nomes: Vec<String>,
) -> IntrinsicCall {
    let lista = list_state.next_list_handle;
    list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
    list_state.lists_verso.insert(lista, nomes);
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_OK);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::ListVerso(lista));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}

/// `Resultado.Erro(causa)`. A causa é sempre `verso`.
fn resultado_erro(map_state: &mut RuntimeMapState, causa: String) -> IntrinsicCall {
    let handle = novo_leque(map_state, crate::falha_operacional::TAG_ERRO);
    if let Some((_, cargas)) = map_state.enum_values.get_mut(&handle) {
        cargas.push(RuntimeEnumPayload::Str(causa));
    }
    IntrinsicCall::Done(Some(RuntimeValue::Int(handle)))
}
// @pinker-nav:end interpreter.falha-operacional.construcao

// @pinker-nav:start interpreter.memoria.estado-enderecavel
// @pinker-nav:domain memoria
// @pinker-nav:layer interpreter
// @pinker-nav:summary O interpretador mantém dois domínios endereçáveis relacionados porém contabilmente distintos: identidades de memória pública, com limites, reserva, vivacidade e contabilidade pelo contrato compartilhado; e o domínio interno monotônico de bindings de união, com limites, snapshots e cópias de payload próprios, que não consomem identidade pública nem podem ser liberados por liberar. Reúne também a representação escalar, o acesso em bytes, a contenção por intervalos, a validação de proveniência e as implementações de alocar e liberar. Não é aleatoriedade.
const PUBLIC_MEMORY_BASE: usize = 0x5000_0000;
const PUBLIC_MEMORY_MAX_IDENTITIES: usize = pinker_memory_contract::MAX_PUBLIC_IDENTITIES as usize;
const PUBLIC_MEMORY_MAX_VIRTUAL_BYTES: usize =
    pinker_memory_contract::MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES as usize;
const PUBLIC_MEMORY_MAX_METADATA_BYTES: usize =
    pinker_memory_contract::MAX_PUBLIC_METADATA_BYTES as usize;
const PUBLIC_MEMORY_MAX_QUARANTINE_BYTES: usize = 0;

/// Base da arena interna de binding de extração de união.
///
/// Fica **acima** do fim da arena pública (`PUBLIC_MEMORY_BASE + 8 GiB`), de
/// modo que nenhum endereço interno possa ser confundido com uma identidade
/// pública nem colidir com ela.
const UNION_BINDING_BASE: usize = 0x4_0000_0000;

/// Tetos aplicados à memória pública.
///
/// São um campo do estado, e não constantes lidas diretamente, para que os
/// testes do próprio módulo possam exercitar o esgotamento com limites
/// reduzidos. Não há variável de ambiente nem opção pública: fora dos testes o
/// valor é sempre [`PUBLIC_MEMORY_LIMITS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PublicMemoryLimits {
    max_identities: usize,
    max_virtual_bytes: usize,
    max_single_reserved_bytes: usize,
    max_live_reserved_bytes: usize,
    max_metadata_bytes: usize,
}

const PUBLIC_MEMORY_LIMITS: PublicMemoryLimits = PublicMemoryLimits {
    max_identities: PUBLIC_MEMORY_MAX_IDENTITIES,
    max_virtual_bytes: PUBLIC_MEMORY_MAX_VIRTUAL_BYTES,
    max_single_reserved_bytes: pinker_memory_contract::MAX_PUBLIC_SINGLE_RESERVED_BYTES as usize,
    max_live_reserved_bytes: pinker_memory_contract::MAX_PUBLIC_LIVE_RESERVED_BYTES as usize,
    max_metadata_bytes: PUBLIC_MEMORY_MAX_METADATA_BYTES,
};

impl PublicMemoryLimits {
    fn contract(self) -> ContractMemoryLimits {
        ContractMemoryLimits {
            max_identities: self.max_identities as u64,
            max_lifetime_virtual_bytes: self.max_virtual_bytes as u64,
            max_single_reserved_bytes: self.max_single_reserved_bytes as u64,
            max_live_reserved_bytes: self.max_live_reserved_bytes as u64,
            max_metadata_bytes: self.max_metadata_bytes as u64,
        }
    }
}

/// Tetos do domínio interno de binding de extração de união.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnionBindingLimits {
    max_regions: u64,
    max_bytes: u64,
}

const UNION_BINDING_LIMITS: UnionBindingLimits = UnionBindingLimits {
    max_regions: crate::union_payload::MAX_UNION_BINDING_REGIONS,
    max_bytes: crate::union_payload::MAX_UNION_BINDING_BYTES,
};

#[derive(Clone, Debug)]
struct PublicMemoryRegion {
    base: usize,
    size: usize,
    reserved: usize,
    alive: bool,
}

/// Arena interna que materializa os bindings de extração de payload agregado.
///
/// É um domínio **separado** do registro de identidades públicas: as regiões
/// aqui não vêm de `alocar`, `liberar` não as aceita, e a cota vitalícia de
/// identidades públicas não é tocada por elas. O storage é monotônico enquanto
/// não existir contrato de desalocação para uniões — por isso possui teto
/// próprio e diagnóstico próprio.
///
/// A arena existe **só aqui**. O backend nativo materializa o mesmo binding num
/// slot do frame reservado no prólogo, reaproveitado a cada passagem pelo ponto
/// de extração: lá não há cota de binding a esgotar nem diagnóstico a emitir. A
/// paridade que os dois back-ends mantêm é a de contabilidade — extrair não
/// consome identidade pública —, não a de capacidade de extração repetida.
#[derive(Clone, Debug)]
struct UnionBindingArena {
    next_address: usize,
    regions: Vec<PublicMemoryRegion>,
    bytes: u64,
    limits: UnionBindingLimits,
}

impl Default for UnionBindingArena {
    fn default() -> Self {
        Self {
            next_address: UNION_BINDING_BASE,
            regions: Vec::new(),
            bytes: 0,
            limits: UNION_BINDING_LIMITS,
        }
    }
}

#[derive(Clone, Debug)]
struct PublicMemoryState {
    next_address: usize,
    budget: PublicMemoryBudget,
    /// Registro de **identidades públicas**. Só `alocar` acrescenta uma entrada
    /// aqui, e cada chamada bem-sucedida acrescenta exatamente uma.
    regions: Vec<PublicMemoryRegion>,
    payload: HashMap<usize, RuntimeValue>,
    limits: PublicMemoryLimits,
    /// Domínio interno de união. Endereçável pelo programa, mas fora da cota
    /// pública e fora do alcance de `liberar`.
    union_bindings: UnionBindingArena,
}

impl Default for PublicMemoryState {
    fn default() -> Self {
        Self {
            next_address: PUBLIC_MEMORY_BASE,
            budget: PublicMemoryBudget::default(),
            regions: Vec::new(),
            payload: HashMap::new(),
            limits: PUBLIC_MEMORY_LIMITS,
            union_bindings: UnionBindingArena::default(),
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

/// Localiza a região endereçável que contém `address`.
///
/// Cobre os **dois** domínios de storage endereçável do interpretador: as
/// identidades públicas criadas por `alocar` e as regiões internas de binding
/// de extração de união. Ambas são memória legítima do ponto de vista de um
/// `deref`, exatamente como no nativo, onde o binding é um slot do frame e o
/// acesso é uma instrução de memória comum. A distinção entre os domínios não
/// está no acesso: está na contabilidade (só o primeiro consome identidade
/// pública) e em `liberar` (que só aceita o primeiro).
fn public_memory_region(state: &PublicMemoryState, address: usize) -> Option<(usize, usize, bool)> {
    let dominios = [&state.regions, &state.union_bindings.regions];
    for regions in dominios {
        for region in regions.iter().rev() {
            if region
                .base
                .checked_add(region.size)
                .is_some_and(|end| address >= region.base && address < end)
            {
                return Some((region.base, region.size, region.alive));
            }
        }
    }
    for regions in dominios {
        for region in regions.iter().rev() {
            if region
                .base
                .checked_add(region.size)
                .is_some_and(|end| address == end)
            {
                return Some((region.base, region.size, region.alive));
            }
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
    // Os dois domínios endereçáveis, na mesma ordem de `public_memory_region`:
    // identidades públicas primeiro, storage interno de binding depois.
    let dominios = [&state.regions, &state.union_bindings.regions];
    Ok(dominios.into_iter().find_map(|regions| {
        regions.iter().rev().find_map(|region| {
            let region_end = region.base.checked_add(region.size)?;
            ((address >= region.base && address <= region_end)
                || (address < region.base && access_end > region.base))
                .then_some((region.base, region.size, region.alive))
        })
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

/// Reserva storage no domínio **interno** de união para o binding de extração.
///
/// Esta é a autoridade única do domínio interno de binding no interpretador:
/// reserva a região, contabiliza os bytes, verifica overflow e emite o
/// diagnóstico. Nada aqui toca o registro de identidades públicas — no nativo o
/// mesmo storage é um slot do frame (`leaq -offset(%rbp)`), que também não
/// consome identidade pública. O arredondamento para 16 bytes garante o maior
/// alinhamento suportado ([`crate::union_payload::MAX_UNION_PAYLOAD_ALIGN`]).
fn union_reserve_binding_storage(
    state: &mut PublicMemoryState,
    size: u64,
    align: u64,
) -> Result<usize, PinkerError> {
    if align > crate::union_payload::MAX_UNION_PAYLOAD_ALIGN {
        return Err(runtime_err(
            "E-RUNTIME-UNION-ALIGN: alinhamento de payload de união acima do suportado",
        ));
    }
    let arena = &mut state.union_bindings;
    let regions = u64::try_from(arena.regions.len()).map_err(|_| {
        runtime_err(
            "E-RUNTIME-UNION-BINDING-METADATA: contagem de bindings de união excede a plataforma",
        )
    })?;
    if regions >= arena.limits.max_regions {
        return Err(runtime_err(
            "E-RUNTIME-UNION-BINDING-BUDGET: orçamento de bindings de extração de união esgotado",
        ));
    }
    let bytes = arena.bytes.checked_add(size).ok_or_else(|| {
        runtime_err(
            "E-RUNTIME-UNION-BINDING-OVERFLOW: overflow no orçamento de bytes de binding de união",
        )
    })?;
    if bytes > arena.limits.max_bytes {
        return Err(runtime_err(
            "E-RUNTIME-UNION-BINDING-BYTES: orçamento de bytes de binding de extração de união esgotado",
        ));
    }
    let size = usize::try_from(size).map_err(|_| {
        runtime_err(
            "E-RUNTIME-UNION-BINDING-OVERFLOW: tamanho de payload de união excede a plataforma",
        )
    })?;
    let rounded = size
        .checked_add(15)
        .map(|value| value & !15)
        .ok_or_else(|| {
            runtime_err(
                "E-RUNTIME-UNION-BINDING-OVERFLOW: overflow ao alinhar storage de binding de união",
            )
        })?;
    let base = arena.next_address;
    let next = base.checked_add(rounded).ok_or_else(|| {
        runtime_err(
            "E-RUNTIME-UNION-BINDING-OVERFLOW: overflow de endereço no domínio interno de união",
        )
    })?;
    arena.regions.try_reserve(1).map_err(|_| {
        runtime_err(
            "E-RUNTIME-UNION-BINDING-METADATA: registro de bindings de união não pôde reservar \
             metadata",
        )
    })?;
    arena.next_address = next;
    arena.bytes = bytes;
    arena.regions.push(PublicMemoryRegion {
        base,
        size,
        reserved: rounded,
        alive: true,
    });
    Ok(base)
}

/// Copia a origem de uma injeção de união para um snapshot independente.
///
/// Para agregados, a origem tem de ser um endereço de uma região pública viva
/// que contenha integralmente `[addr, addr + size)`. Origem incompleta,
/// liberada, desalinhada ou fora de região é recusada com diagnóstico — nunca
/// aceita parcialmente nem guardada como ponteiro.
fn union_snapshot_from_source(
    payload: RuntimeValue,
    layout: crate::union_payload::UnionPayloadLayout,
    public_memory_state: &PublicMemoryState,
) -> Result<UnionPayloadSnapshot, PinkerError> {
    use crate::union_payload::UnionPayloadRepresentation;
    match layout.representation {
        UnionPayloadRepresentation::Scalar => Ok(UnionPayloadSnapshot::Scalar(payload)),
        UnionPayloadRepresentation::OpaqueHandle => Ok(UnionPayloadSnapshot::OpaqueHandle(payload)),
        UnionPayloadRepresentation::Aggregate => {
            let RuntimeValue::Ptr(address) = payload else {
                return Err(runtime_err(
                    "E-RUNTIME-UNION-AGGREGATE-SOURCE: agregado de união exige valor representado \
                     por endereço",
                ));
            };
            let size = usize::try_from(layout.size)
                .map_err(|_| runtime_err("tamanho de agregado de união excede a plataforma"))?;
            let align = usize::try_from(layout.align)
                .map_err(|_| runtime_err("alinhamento de agregado de união excede a plataforma"))?;
            if align == 0 || address % align != 0 {
                return Err(runtime_err(
                    "E-RUNTIME-UNION-AGGREGATE-SOURCE: origem de agregado de união desalinhada",
                ));
            }
            let region = public_memory_access_region(public_memory_state, address, size)?;
            let Some((base, region_size, alive)) = region else {
                return Err(runtime_err(
                    "E-RUNTIME-UNION-AGGREGATE-SOURCE: origem de agregado de união fora de região \
                     pública conhecida",
                ));
            };
            if !alive {
                return Err(runtime_err(
                    "E-RUNTIME-MEM-USE-AFTER-FREE: uso após liberar detectado em origem de união",
                ));
            }
            if !public_memory_interval_contained(base, region_size, address, size)? {
                return Err(runtime_err(
                    "E-RUNTIME-UNION-AGGREGATE-SOURCE: origem de agregado de união incompleta na \
                     região",
                ));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve(size)
                .map_err(|_| runtime_err("snapshot de união não pôde reservar bytes"))?;
            for offset in 0..size {
                let byte_address = address
                    .checked_add(offset)
                    .ok_or_else(|| runtime_err("overflow ao copiar agregado de união"))?;
                // Byte ausente é byte zero, exatamente como na leitura pública:
                // o padding é preservado com a mesma regra.
                let byte = match public_memory_state.payload.get(&byte_address) {
                    Some(RuntimeValue::Int(value)) => (*value & u8::MAX as u64) as u8,
                    None => 0,
                    Some(_) => {
                        return Err(runtime_err(
                            "representação interna inválida em byte de agregado de união",
                        ));
                    }
                };
                bytes.push(byte);
            }
            Ok(UnionPayloadSnapshot::Aggregate { bytes })
        }
    }
}

/// Materializa o binding de um braço a partir do snapshot imutável.
///
/// Escalares e handles são clonados. Agregados recebem uma região nova, para a
/// qual o snapshot é copiado: o ponteiro devolvido nunca é o storage interno do
/// descritor, e duas extrações devolvem regiões distintas.
fn union_snapshot_to_binding(
    snapshot: &UnionPayloadSnapshot,
    layout: crate::union_payload::UnionPayloadLayout,
    payload_type: TypeIR,
    public_memory_state: &mut PublicMemoryState,
) -> Result<RuntimeValue, PinkerError> {
    match snapshot {
        UnionPayloadSnapshot::Scalar(value) | UnionPayloadSnapshot::OpaqueHandle(value) => {
            coerce_runtime_value_to_type(value.clone(), payload_type)
        }
        UnionPayloadSnapshot::Aggregate { bytes } => {
            let expected = usize::try_from(layout.size)
                .map_err(|_| runtime_err("tamanho de agregado de união excede a plataforma"))?;
            if bytes.len() != expected {
                return Err(runtime_err(
                    "snapshot de agregado de união com tamanho divergente",
                ));
            }
            let base =
                union_reserve_binding_storage(public_memory_state, layout.size, layout.align)?;
            for (offset, byte) in bytes.iter().enumerate() {
                let byte_address = base
                    .checked_add(offset)
                    .ok_or_else(|| runtime_err("overflow ao materializar agregado de união"))?;
                public_memory_state
                    .payload
                    .insert(byte_address, RuntimeValue::Int(u64::from(*byte)));
            }
            Ok(RuntimeValue::Ptr(base))
        }
    }
}
fn public_memory_allocate(
    args: &[RuntimeValue],
    state: &mut PublicMemoryState,
) -> Result<IntrinsicCall, PinkerError> {
    debug_assert_eq!(PUBLIC_MEMORY_MAX_QUARANTINE_BYTES, 0);
    let [RuntimeValue::Int(size)] = args else {
        return Err(runtime_err("'alocar' exige um tamanho 'u64' em bytes"));
    };
    let reservation = match reserve_public_allocation(
        state.budget,
        *size,
        isize::MAX as usize,
        state.limits.contract(),
    ) {
        PublicAllocationVerdict::Allowed(reservation) => reservation,
        verdict => {
            return Err(runtime_err(
                verdict
                    .diagnostic()
                    .expect("todo veredicto recusado possui diagnóstico"),
            ))
        }
    };
    let base = state.next_address;
    let next = base
        .checked_add(reservation.reserved_page_bytes)
        .ok_or_else(|| {
            runtime_err(
                PublicAllocationVerdict::CounterOverflow
                    .diagnostic()
                    .expect("diagnóstico de overflow"),
            )
        })?;
    state.regions.try_reserve(1).map_err(|_| {
        runtime_err(
            PublicAllocationVerdict::MetadataBudgetExceeded
                .diagnostic()
                .expect("diagnóstico de metadata"),
        )
    })?;
    state.regions.push(PublicMemoryRegion {
        base,
        size: reservation.logical_bytes,
        reserved: reservation.reserved_page_bytes,
        alive: true,
    });
    state.next_address = next;
    state.budget = reservation.next_budget;
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
    if let Some(index) = state
        .regions
        .iter()
        .rposition(|region| region.base == *pointer)
    {
        if !state.regions[index].alive {
            return Err(runtime_err(
                "E-RUNTIME-MEM-DOUBLE-FREE: 'liberar' detectou double free",
            ));
        }
        let region = state.regions[index].clone();
        let base = region.base;
        let end = base
            .checked_add(region.size)
            .ok_or_else(|| runtime_err("overflow em metadata de memória pública"))?;
        let next_budget = release_public_live_bytes(state.budget, region.reserved)
            .ok_or_else(|| runtime_err("underflow no orçamento público vivo"))?;
        state
            .payload
            .retain(|address, _| *address < base || *address >= end);
        state.regions[index].alive = false;
        state.budget = next_budget;
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
// @pinker-nav:end interpreter.memoria.estado-enderecavel

// @pinker-nav:start interpreter.hospedeiro.servicos-auxiliares
// @pinker-nav:domain hospedeiro
// @pinker-nav:layer interpreter
// @pinker-nav:summary Reúne helpers hospedados usados pelas intrínsecas para stdin, aleatoriedade, ambiente, formatação textual, CSV, JSON mínimo, tempo UTC e processos; encapsula efeitos e normalizações auxiliares sem criar novas ferramentas da Trama nem alterar a semântica do dispatcher, e concentra em comando_de_processo a construção de todo Command das famílias de subprocesso, que instala um pre_exec devolvendo SIGPIPE a SIG_DFL no filho antes do exec em paridade com o runtime nativo. A leitura de argumento nomeado deixou de morar aqui: sobraram os dois guardas de chave vazia, e eles só escolhem qual mensagem de `pinker_argv_contract` usar — a classificação da chave é da autoridade compartilhada com o runtime nativo (#492).
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
        return Err(runtime_err(&pinker_argv_contract::mensagem_chave_vazia(
            intrinsic_name,
        )));
    }
    Ok(())
}

fn ensure_env_key_valid(intrinsic_name: &str, key: &str) -> Result<(), PinkerError> {
    if key.is_empty() {
        return Err(runtime_err(
            &pinker_argv_contract::mensagem_chave_ambiente_vazia(intrinsic_name),
        ));
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

/// Recorte plano histórico, agora projetado sobre a autoridade compartilhada.
///
/// O cursor próprio desta superfície deixou de existir. Ele e a gramática
/// adulta eram duas implementações da mesma linguagem, capazes de divergir de
/// novo — e divergir foi exatamente o defeito que a Parte E1 veio fechar.
///
/// O domínio numérico continua `u64`: `i64::MAX + 1 ..= u64::MAX` pertence a
/// esta superfície e **não** ao modelo adulto. Uma gramática, duas projeções.
fn parse_json_plano_bombom(json: &str) -> Result<HashMap<String, u64>, PinkerError> {
    pinker_json_contract::interpretar_plano_bombom(json)
        .map(|pares| pares.into_iter().collect())
        .map_err(|causa| {
            runtime_err(&format!(
                "json inválido em 'ler_json_plano_bombom': {causa}"
            ))
        })
}

/// Emissão plana histórica: chaves em ordem, valores `u64` exatos.
///
/// Sem cast para `i64` em ponto algum — `u64::MAX` sai
/// `18446744073709551615`.
fn emit_json_plano_bombom(mapa: &HashMap<String, u64>) -> Result<String, PinkerError> {
    let pares: Vec<(String, u64)> = mapa
        .iter()
        .map(|(chave, valor)| (chave.clone(), *valor))
        .collect();
    pinker_json_contract::serializar_plano_bombom(&pares).map_err(|causa| {
        runtime_err(&format!(
            "json inválido em 'emitir_json_plano_bombom': {causa}"
        ))
    })
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

#[cfg(unix)]
const SINAL_SIGPIPE: i32 = 13;
#[cfg(unix)]
const SINAL_HANDLER_PADRAO: usize = 0;
#[cfg(unix)]
const SINAL_HANDLER_ERRO: usize = usize::MAX;

#[cfg(unix)]
extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
}

/// Devolve `sinal` à disposição padrão (`SIG_DFL`) no processo corrente.
///
/// Espelha `restaurar_disposicao_padrao` do runtime nativo: é a operação de
/// sistema mínima executada no contexto pré-`exec`, sem alocação, formatação,
/// acesso a ambiente ou lock. `signal(2)` está na lista async-signal-safe da
/// POSIX. Falha vira `io::Error` e chega ao pai como erro de criação do
/// processo.
///
/// # Safety
/// A `unsafe` cobre só a chamada FFI a `signal(2)`: `sinal` é um número de
/// sinal válido, o handler é a constante `SIG_DFL` (nenhum código de usuário
/// passa a ser executável por sinal) e nenhum ponteiro é desreferenciado.
#[cfg(unix)]
fn restaurar_disposicao_padrao(sinal: i32) -> std::io::Result<()> {
    // SAFETY: ver as precondições acima — FFI pura, handler constante, sem
    // ponteiros e sem código de usuário.
    let anterior = unsafe { signal(sinal, SINAL_HANDLER_PADRAO) };
    if anterior == SINAL_HANDLER_ERRO {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Constrói o `Command` comum a todas as famílias de processo do
/// interpretador.
///
/// O processo `pink` ignora `SIGPIPE` (a inicialização da std instala
/// `SIG_IGN`), e `SIG_IGN` sobrevive a `exec`. Para que o contrato observável
/// seja o mesmo dos dois back-ends, este construtor devolve explicitamente
/// `SIGPIPE` a `SIG_DFL` no filho, imediatamente antes do `exec`, em vez de
/// depender de `std::process::Command` fazer isso por conta própria.
pub(crate) fn comando_de_processo(command_name: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(command_name);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: a closure roda no filho entre `fork` e `exec` e executa
        // apenas `restaurar_disposicao_padrao` — uma chamada async-signal-safe
        // a `signal(2)`, sem alocação, formatação, ambiente, lock ou código de
        // usuário.
        unsafe {
            command.pre_exec(|| restaurar_disposicao_padrao(SINAL_SIGPIPE));
        }
    }
    command
}

fn executar_processo_minimo(
    command_name: &str,
    explicit_argv: Option<&str>,
) -> Result<u64, PinkerError> {
    validar_comando_nao_vazio("executar_processo", command_name)?;

    let mut command = comando_de_processo(command_name);
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

    let mut command = comando_de_processo(command_name);
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

    let mut producer = comando_de_processo(producer_name)
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

    let mut consumer = comando_de_processo(consumer_name)
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

    let mut command = comando_de_processo(command_name);
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

    let mut command = comando_de_processo(command_name);
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
        RuntimeValue::MapBombomVerso(_) | RuntimeValue::Map(_) => Err(runtime_err(msg)),
        RuntimeValue::Callable(_) => Err(runtime_err(msg)),
        RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => Err(runtime_err(msg)),
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
        RuntimeValue::MapBombomVerso(_) | RuntimeValue::Map(_) => Err(runtime_err(msg)),
        RuntimeValue::Callable(_) => Err(runtime_err(msg)),
        RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => Err(runtime_err(msg)),
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

    if ty == crate::ir::TypeIR::OpaqueWordHandle {
        // A representação é a mesma palavra para todas as famílias nominais; a
        // categoria acompanha o valor para que uma não seja aceita no lugar da
        // outra por compartilhar largura.
        return match value {
            RuntimeValue::SaidaProcesso(handle) => Ok(RuntimeValue::SaidaProcesso(handle)),
            RuntimeValue::ValorJson(handle) => Ok(RuntimeValue::ValorJson(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado handle opaco nominal",
            )),
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
            RuntimeValue::Map(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::Callable(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::SaidaProcesso(_) | RuntimeValue::ValorJson(_) => Err(runtime_err(
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
    if msg.contains("divisão por zero") {
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
        MachineInstr::UnionTag { .. } => "union_tag",
        MachineInstr::UnionExtract { .. } => "union_extract",
        MachineInstr::BitAnd { .. } => "bitand",
        MachineInstr::BitOr { .. } => "bitor",
        MachineInstr::BitXor { .. } => "bitxor",
        MachineInstr::Shl { .. } => "shl",
        MachineInstr::Shr { .. } => "shr",
        MachineInstr::Add { .. } => "add",
        MachineInstr::PointerOffset { .. } => "pointer_offset",
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
mod tests;
