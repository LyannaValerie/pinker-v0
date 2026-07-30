//! Máquina abstrata de pilha — camada `--machine` do pipeline.
//!
//! `MachineProgram` é o resultado do lowering de `SelectedProgram` para um conjunto explícito
//! de instruções de pilha (`MachineInstr`) e terminadores (`MachineTerminator`).
//! Cada instrução opera sobre uma pilha implícita de valores; nenhuma instrução referencia
//! registradores — apenas slots nomeados e a pilha de operandos.
//!
//! A representação é validada por `abstract_machine_validate` antes de ser interpretada
//! ou emitida como pseudo-assembly via `backend_text`.
//!
//! Posição no pipeline:
//!   `instr_select` → **`abstract_machine`** → `abstract_machine_validate` → `interpreter` / `backend_text`

use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{FalarArgSelected, SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::TypeIR;
use std::collections::HashMap;

// @pinker-nav:start machine.modelo.representacao
// @pinker-nav:domain modelo
// @pinker-nav:layer machine
// @pinker-nav:summary Modelo de dados da máquina abstrata de pilha: programa, globais, funções com slots, blocos, instruções de pilha (`MachineInstr`) e terminadores — a representação executada pelo interpretador.
/// Programa completo na representação de máquina abstrata.
/// Contém globals (constantes somente-leitura) e funções com blocos de instruções de pilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgram {
    pub module_name: String,
    pub union_types: Vec<crate::ir::UnionTypeIR>,
    pub globals: Vec<MachineGlobal>,
    pub functions: Vec<MachineFunction>,
}

/// Constante global somente-leitura. O runtime acessa via `LoadGlobal`; escrita não é suportada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineGlobal {
    pub name: String,
    pub ty: TypeIR,
    pub value: OperandIR,
}

/// Função na Machine. `params` e `locals` listam os nomes dos slots nomeados;
/// `slot_types` mapeia cada slot ao seu tipo para uso pelo validador de pilha.
/// Temporários (`%tN`) são gerados durante o lowering e também registrados em `slot_types`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunction {
    pub name: String,
    pub ret_type: TypeIR,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub slot_types: HashMap<String, TypeIR>,
    pub blocks: Vec<MachineBlock>,
}

/// Bloco básico da Machine: sequência linear de instruções de pilha seguida de um terminador.
/// A invariante é que `terminator` sempre está presente — não existe bloco sem saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineBlock {
    pub label: String,
    pub code: Vec<MachineInstr>,
    pub terminator: MachineTerminator,
}

/// Instruções de pilha. Convenções:
/// - `PushInt`/`PushBool`: empilha literal.
/// - `LoadSlot`/`StoreSlot`: lê/escreve slot nomeado (params, locals ou temporário `%tN`).
/// - `LoadGlobal`: lê constante global pelo nome; não existe `StoreGlobal` nesta versão.
/// - Operações aritméticas/lógicas/comparação: consomem topo(s) da pilha e empilham resultado.
/// - `Call`/`CallVoid`: empilha `argc` argumentos antes da instrução; `Call` empilha o retorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineInstr {
    PushInt(u64),
    PushBool(bool),
    PushStr(String),
    LoadSlot(String),
    LoadGlobal(String),
    StoreSlot(String),
    Neg {
        ty: TypeIR,
    },
    Not,
    BitNot {
        ty: TypeIR,
    },
    DerefLoad {
        ty: TypeIR,
        is_volatile: bool,
    },
    DerefStore {
        ty: TypeIR,
        is_volatile: bool,
    },
    Cast {
        ty: TypeIR,
    },
    MakeUnion {
        union_type_id: crate::ir::UnionTypeId,
        tag: u64,
        /// Identidade semântica do membro, decidida uma única vez no lowering.
        /// A máquina abstrata a transporta como metadado verificável; a
        /// representação do payload na memória não muda por causa dela.
        resolved_member_type_id: crate::ir::ResolvedTypeId,
        canonical_member_key: String,
        payload_type: TypeIR,
        payload_size: u64,
        payload_align: u64,
    },
    // Operações internas tipadas de união na máquina abstrata. Consomem o
    // valor de união no topo da pilha; nunca são chamadas por nome.
    UnionTag {
        union_type_id: crate::ir::UnionTypeId,
    },
    UnionExtract {
        union_type_id: crate::ir::UnionTypeId,
        tag: u64,
        resolved_member_type_id: crate::ir::ResolvedTypeId,
        canonical_member_key: String,
        payload_type: TypeIR,
        payload_size: u64,
        payload_align: u64,
    },
    BitAnd {
        ty: TypeIR,
    },
    BitOr {
        ty: TypeIR,
    },
    BitXor {
        ty: TypeIR,
    },
    Shl {
        ty: TypeIR,
    },
    Shr {
        ty: TypeIR,
    },
    Add {
        ty: TypeIR,
    },
    Sub {
        ty: TypeIR,
    },
    Mul {
        ty: TypeIR,
    },
    Div {
        ty: TypeIR,
    },
    Mod {
        ty: TypeIR,
    },
    CmpEq {
        ty: TypeIR,
    },
    CmpNe {
        ty: TypeIR,
    },
    CmpLt {
        ty: TypeIR,
    },
    CmpLe {
        ty: TypeIR,
    },
    CmpGt {
        ty: TypeIR,
    },
    CmpGe {
        ty: TypeIR,
    },
    Call {
        callee: String,
        argc: usize,
    },
    CallVoid {
        callee: String,
        argc: usize,
    },
    // Fase 242: empilha o handle callable (descritor estático) da função
    // top-level nomeada.
    PushFunctionRef(String),
    // Fase 245: empilha uma palavra com o endereço cru do símbolo de código.
    PushRawFunctionRef(String),
    // Fase 242: consome do topo o handle callable e, abaixo dele, `argc`
    // argumentos; sempre produz valor (tipo função público nunca é `nulo`).
    CallIndirect {
        argc: usize,
    },
    // Fase 245: consome um endereço cru e argumentos, sem descritor/__env.
    CallRaw {
        argc: usize,
        has_return: bool,
    },
    // Fase 243: consome do topo `capture_count` valores (snapshot por
    // valor, já empilhados na ordem de primeira referência), aloca o
    // ambiente em heap quando `capture_count > 0` e empilha o handle
    // callable da nova instância de closure (nunca memoizado — cada
    // criação do literal produz um ambiente próprio).
    MakeClosure {
        function_name: String,
        capture_count: usize,
    },
    // Fase 244: consome um valor concreto e produz o handle público
    // de uma palavra. O descritor e o snapshot serão materializados
    // pelo runtime executável da etapa seguinte.
    MakeTraitObject {
        trait_name: String,
        concrete_type: TypeIR,
        concrete_type_name: String,
        concrete_size: u64,
        vtable_methods: Vec<String>,
    },
    // Fase 244: os argumentos de usuário são empilhados primeiro e o
    // handle do objeto por último. A instrução consome o objeto do topo,
    // depois `argc` argumentos, e produz valor somente quando o retorno
    // não é `nulo`.
    TraitCall {
        trait_name: String,
        method_name: String,
        method_slot: u64,
        method_count: u64,
        argc: usize,
        param_types: Vec<TypeIR>,
        ret_type: TypeIR,
    },
    PrintIntInline,
    PrintBoolInline,
    PrintStrValueInline,
    PrintStrInline(String),
    PrintSpace,
    PrintNewline,
    InlineAsm {
        chunks: Vec<String>,
        span: crate::token::Span,
    },
}

/// Terminadores de bloco. `BrTrue` consome o topo da pilha (deve ser `lógica`).
/// `Ret` consome o único valor da pilha como retorno; `RetVoid` exige pilha vazia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineTerminator {
    Jmp(String),
    BrTrue {
        then_label: String,
        else_label: String,
    },
    Ret,
    RetVoid,
}
// @pinker-nav:end machine.modelo.representacao

// @pinker-nav:start machine.lowering.programa-blocos
// @pinker-nav:domain lowering
// @pinker-nav:layer machine
// @pinker-nav:summary Entrada da máquina abstrata de pilha: converte `SelectedProgram` em `MachineProgram`, copiando as globais, transformando cada função (sequência de instruções por bloco via `lower_instr`, terminador via `lower_term`) e preservando parâmetros, locais e `slot_types`. Nota: `slot_types` é apenas copiado da seleção (parâmetros+locais) — os temporários `%tN` não são acrescentados aqui, embora o doc de `MachineFunction` afirme o contrário; e `is_freestanding` do `SelectedProgram` não é propagado ao `MachineProgram`.
pub fn lower_program(selected: &SelectedProgram) -> Result<MachineProgram, PinkerError> {
    let globals = selected
        .globals
        .iter()
        .map(|g| MachineGlobal {
            name: g.name.clone(),
            ty: g.ty,
            value: g.value.clone(),
        })
        .collect();

    let functions = selected
        .functions
        .iter()
        .map(|f| -> Result<MachineFunction, PinkerError> {
            let blocks = f
                .blocks
                .iter()
                .map(|b| -> Result<MachineBlock, PinkerError> {
                    let mut code = Vec::new();

                    for instruction in &b.instructions {
                        lower_instr(instruction, &mut code)?;
                    }

                    let terminator = lower_term(&b.terminator, &mut code);

                    Ok(MachineBlock {
                        label: b.label.clone(),
                        code,
                        terminator,
                    })
                })
                .collect::<Result<Vec<_>, PinkerError>>()?;

            Ok(MachineFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.clone(),
                locals: f.locals.clone(),
                slot_types: f.slot_types.clone(),
                blocks,
            })
        })
        .collect::<Result<Vec<_>, PinkerError>>()?;

    Ok(MachineProgram {
        module_name: selected.module_name.clone(),
        union_types: selected.union_types.clone(),
        globals,
        functions,
    })
}
// @pinker-nav:end machine.lowering.programa-blocos

// @pinker-nav:start machine.lowering.instrucoes-pilha
// @pinker-nav:domain lowering
// @pinker-nav:layer machine
// @pinker-nav:summary Dispatcher `lower_instr` que converte cada `SelectedInstr` em operações da máquina de pilha seguindo o padrão carregar operandos → emitir a operação → armazenar o resultado em `StoreSlot("%tN")` quando há destino: `Mov`, unários, `DerefLoad`/`DerefStore`, `Cast`, bitwise, aritmética, comparações, chamadas (`Call`/`CallVoid` empilham os argumentos) e a emissão de `falar` (via `lower_falar_arg`, distinguindo string literal, `verso`, `lógica` e inteiro). Os `%tN` são slots nomeados de resultado — não são registradores físicos; não há SSA nem ABI de hardware.
fn lower_instr(inst: &SelectedInstr, code: &mut Vec<MachineInstr>) -> Result<(), PinkerError> {
    match inst {
        SelectedInstr::Mov { dest, src } => {
            emit_load(src, code);
            code.push(MachineInstr::StoreSlot(dest.clone()));
        }
        SelectedInstr::Neg { dest, operand, ty } => {
            emit_load(operand, code);
            code.push(MachineInstr::Neg { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Not { dest, operand } => {
            emit_load(operand, code);
            code.push(MachineInstr::Not);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitNot { dest, operand, ty } => {
            emit_load(operand, code);
            code.push(MachineInstr::BitNot { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::DerefLoad {
            dest,
            ptr,
            ty,
            is_volatile,
        } => {
            emit_load(ptr, code);
            code.push(MachineInstr::DerefLoad {
                ty: *ty,
                is_volatile: *is_volatile,
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::DerefStore {
            ptr,
            value,
            ty,
            is_volatile,
        } => {
            emit_load(ptr, code);
            emit_load(value, code);
            code.push(MachineInstr::DerefStore {
                ty: *ty,
                is_volatile: *is_volatile,
            });
        }
        SelectedInstr::Cast {
            dest,
            value,
            target_type,
        } => {
            emit_load(value, code);
            code.push(MachineInstr::Cast { ty: *target_type });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::UnionInject {
            dest,
            value,
            union_type_id,
            tag,
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_size,
            payload_align,
        } => {
            emit_load(value, code);
            code.push(MachineInstr::MakeUnion {
                union_type_id: *union_type_id,
                tag: *tag,
                resolved_member_type_id: *resolved_member_type_id,
                canonical_member_key: canonical_member_key.clone(),
                payload_type: *payload_type,
                payload_size: *payload_size,
                payload_align: *payload_align,
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::UnionTag {
            dest,
            value,
            union_type_id,
        } => {
            emit_load(value, code);
            code.push(MachineInstr::UnionTag {
                union_type_id: *union_type_id,
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::UnionExtract {
            dest,
            value,
            union_type_id,
            tag,
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_size,
            payload_align,
        } => {
            emit_load(value, code);
            code.push(MachineInstr::UnionExtract {
                union_type_id: *union_type_id,
                tag: *tag,
                resolved_member_type_id: *resolved_member_type_id,
                canonical_member_key: canonical_member_key.clone(),
                payload_type: *payload_type,
                payload_size: *payload_size,
                payload_align: *payload_align,
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitAnd { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitAnd { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitOr { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitOr { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitXor { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitXor { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Shl { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Shl { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Shr { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Shr { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Add { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Add { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Sub { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Sub { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Mul { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Mul { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Div { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Div { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Mod { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Mod { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpEq { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpEq { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpNe { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpNe { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLt { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLt { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLe { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLe { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGt { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGt { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGe { dest, lhs, rhs, ty } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGe { ty: *ty });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Call {
            dest, callee, args, ..
        } => {
            for arg in args {
                emit_load(arg, code);
            }
            code.push(MachineInstr::Call {
                callee: callee.clone(),
                argc: args.len(),
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CallVoid { callee, args } => {
            for arg in args {
                emit_load(arg, code);
            }
            code.push(MachineInstr::CallVoid {
                callee: callee.clone(),
                argc: args.len(),
            });
        }
        SelectedInstr::CallIndirect {
            dest, callee, args, ..
        } => {
            for arg in args {
                emit_load(arg, code);
            }
            emit_load(callee, code);
            code.push(MachineInstr::CallIndirect { argc: args.len() });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CallRaw {
            dest, callee, args, ..
        } => {
            for arg in args {
                emit_load(arg, code);
            }
            emit_load(callee, code);
            code.push(MachineInstr::CallRaw {
                argc: args.len(),
                has_return: dest.is_some(),
            });
            if let Some(dest) = dest {
                code.push(MachineInstr::StoreSlot(temp_name(*dest)));
            }
        }
        SelectedInstr::MakeClosure {
            dest,
            function_name,
            captures,
        } => {
            for capture in captures {
                emit_load(capture, code);
            }
            code.push(MachineInstr::MakeClosure {
                function_name: function_name.clone(),
                capture_count: captures.len(),
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::MakeTraitObject {
            dest,
            value,
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => {
            emit_load(value, code);
            code.push(MachineInstr::MakeTraitObject {
                trait_name: trait_name.clone(),
                concrete_type: *concrete_type,
                concrete_type_name: concrete_type_name.clone(),
                concrete_size: *concrete_size,
                vtable_methods: vtable_methods.clone(),
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::TraitCall {
            dest,
            object,
            trait_name,
            method_name,
            method_slot,
            method_count,
            args,
            param_types,
            ret_type,
        } => {
            for arg in args {
                emit_load(arg, code);
            }

            // O objeto fica no topo. A operação o consome antes dos
            // argumentos e acrescentará o snapshot como primeiro receiver
            // somente no runtime, sem reutilizar `CallIndirect`.
            emit_load(object, code);

            code.push(MachineInstr::TraitCall {
                trait_name: trait_name.clone(),
                method_name: method_name.clone(),
                method_slot: *method_slot,
                method_count: *method_count,
                argc: args.len(),
                param_types: param_types.clone(),
                ret_type: *ret_type,
            });

            if let Some(dest) = dest {
                code.push(MachineInstr::StoreSlot(temp_name(*dest)));
            }
        }
        SelectedInstr::Falar { args } => {
            for (idx, arg) in args.iter().enumerate() {
                if idx > 0 {
                    code.push(MachineInstr::PrintSpace);
                }
                lower_falar_arg(arg, code);
            }
            code.push(MachineInstr::PrintNewline);
        }
        SelectedInstr::InlineAsm { chunks, span } => {
            code.push(MachineInstr::InlineAsm {
                chunks: chunks.clone(),
                span: *span,
            });
        }
    }

    Ok(())
}

fn lower_falar_arg(arg: &FalarArgSelected, code: &mut Vec<MachineInstr>) {
    match &arg.value {
        OperandIR::Str(s) => code.push(MachineInstr::PrintStrInline(s.clone())),
        _ if arg.ty == TypeIR::Verso => {
            emit_load(&arg.value, code);
            code.push(MachineInstr::PrintStrValueInline);
        }
        _ if arg.ty == TypeIR::Logica => {
            emit_load(&arg.value, code);
            code.push(MachineInstr::PrintBoolInline);
        }
        _ => {
            emit_load(&arg.value, code);
            code.push(MachineInstr::PrintIntInline);
        }
    }
}

// @pinker-nav:end machine.lowering.instrucoes-pilha

// @pinker-nav:start machine.lowering.terminadores
// @pinker-nav:domain lowering
// @pinker-nav:layer machine
// @pinker-nav:summary `lower_term` converte cada `SelectedTerminator` em `MachineTerminator`: `Jmp` direto; `Br` carrega a condição na pilha antes de `BrTrue`; `Ret(Some)` carrega o valor de retorno antes de `Ret`; `Ret(None)` vira `RetVoid`. Os rótulos vêm do CFG — não há reconstrução de fluxo.
fn lower_term(term: &SelectedTerminator, code: &mut Vec<MachineInstr>) -> MachineTerminator {
    match term {
        SelectedTerminator::Jmp(label) => MachineTerminator::Jmp(label.clone()),
        SelectedTerminator::Br {
            cond,
            then_label,
            else_label,
        } => {
            emit_load(cond, code);
            MachineTerminator::BrTrue {
                then_label: then_label.clone(),
                else_label: else_label.clone(),
            }
        }
        SelectedTerminator::Ret(Some(v)) => {
            emit_load(v, code);
            MachineTerminator::Ret
        }
        SelectedTerminator::Ret(None) => MachineTerminator::RetVoid,
    }
}

// @pinker-nav:end machine.lowering.terminadores

// @pinker-nav:start machine.lowering.operandos-slots
// @pinker-nav:domain lowering
// @pinker-nav:layer machine
// @pinker-nav:summary Tradução de cada `OperandIR` numa carga da pilha: literais inteiro/lógico/string viram `PushInt`/`PushBool`/`PushStr`; local e global viram `LoadSlot`/`LoadGlobal`; temporário vira `LoadSlot(temp_name)`, e `temp_name` produz o nome canônico `%tN` reconhecido pelo validador. Não faz inferência de tipos nem validação de pilha.
fn emit_load(op: &OperandIR, code: &mut Vec<MachineInstr>) {
    match op {
        OperandIR::Int(v) => code.push(MachineInstr::PushInt(*v)),
        OperandIR::Bool(v) => code.push(MachineInstr::PushBool(*v)),
        OperandIR::Str(s) => code.push(MachineInstr::PushStr(s.clone())),
        OperandIR::Local(s) => code.push(MachineInstr::LoadSlot(s.clone())),
        OperandIR::GlobalConst(g) => code.push(MachineInstr::LoadGlobal(g.clone())),
        OperandIR::Temp(t) => code.push(MachineInstr::LoadSlot(temp_name(*t))),
        OperandIR::FunctionRef(name) => code.push(MachineInstr::PushFunctionRef(name.clone())),
        OperandIR::RawFunctionRef(name) => {
            code.push(MachineInstr::PushRawFunctionRef(name.clone()))
        }
    }
}

// Temporários recebem o nome canônico `%tN` (N = índice do TempIR).
// Esse padrão é reconhecido pelo validador em `abstract_machine_validate::is_temp_slot`.
fn temp_name(t: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", t.0)
}
// @pinker-nav:end machine.lowering.operandos-slots

// @pinker-nav:start machine.renderizacao.programa
// @pinker-nav:domain renderizacao
// @pinker-nav:layer machine
// @pinker-nav:summary `render_program`: forma textual do `MachineProgram` ao nível de módulo, globais e cada função (parâmetros, locais, descoberta e exibição dos temporários, blocos com instruções e terminador), delegando a formatação de cada elemento aos helpers de componentes e apresentação. Recebe a máquina pronta; não abaixa de novo, não valida nem executa.
pub fn render_program(program: &MachineProgram) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));
    line(&mut out, 0, "globals:");
    if program.globals.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for g in &program.globals {
            line(
                &mut out,
                1,
                &format!("global @{} = {}", g.name, render_operand(&g.value)),
            );
        }
    }

    line(&mut out, 0, "machine:");
    for f in &program.functions {
        line(&mut out, 1, &format!("func {}:", f.name));

        // Parâmetros: exibe nomes limpos (sem prefixo interno %nome#N → nome)
        line(
            &mut out,
            2,
            &format!(
                "params {}",
                if f.params.is_empty() {
                    "[]".to_string()
                } else {
                    f.params
                        .iter()
                        .map(|p| clean_slot_display(p))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        );

        // Locais do usuário: exibe nomes limpos
        line(
            &mut out,
            2,
            &format!(
                "locals {}",
                if f.locals.is_empty() {
                    "[]".to_string()
                } else {
                    f.locals
                        .iter()
                        .map(|l| clean_slot_display(l))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        );

        // Temporários internos: slots %tN gerados pelo compilador, não visíveis no fonte Pinker.
        // Coletados varrendo StoreSlot nos blocos (não estão em slot_types).
        let temps: Vec<String> = {
            let mut seen = HashMap::new();
            for b in &f.blocks {
                for instr in &b.code {
                    if let MachineInstr::StoreSlot(s) = instr {
                        if is_render_temp(s) {
                            seen.insert(s.clone(), ());
                        }
                    }
                }
            }
            let mut v: Vec<String> = seen.into_keys().collect();
            v.sort();
            v
        };
        if !temps.is_empty() {
            line(
                &mut out,
                2,
                &format!("temps  {}  ; gerados pelo compilador", temps.join(", ")),
            );
        }

        for b in &f.blocks {
            // Rótulo do bloco com anotação de papel quando reconhecível
            line(
                &mut out,
                2,
                &format!("{}:{}", b.label, block_role_annotation(&b.label)),
            );
            for i in &b.code {
                line(&mut out, 3, &format!("vm {}", render_instr(i)));
            }
            line(
                &mut out,
                3,
                &format!("term {}", render_term(&b.label, &b.terminator)),
            );
        }
    }

    out
}
// @pinker-nav:end machine.renderizacao.programa

// @pinker-nav:start machine.renderizacao.apresentacao
// @pinker-nav:domain renderizacao
// @pinker-nav:layer machine
// @pinker-nav:summary Apresentação humana dos nomes e blocos na renderização: `clean_slot_display` limpa `%nome#N` para a forma legível preservando `%tN`, `is_render_temp` distingue os temporários internos, e `block_role_annotation` anota o papel de cada bloco (entry, ramos, laços, joins, curto-circuito) por convenções de prefixo de label. É apresentação, não lowering; os nomes limpos e as anotações não voltam para o modelo interno nem são metadados semânticos persistidos.
// Converte nome interno de slot para forma legível ao usuário.
// `%varname#0` → `varname`; `%t0` permanece `%t0` (temporário interno).
fn clean_slot_display(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('%') {
        // Temporário interno: %tN — mantém forma original para distinção visual
        if is_render_temp(s) {
            return s.to_string();
        }
        // Local/param do usuário: %nome#N → nome
        if let Some(pos) = rest.rfind('#') {
            return rest[..pos].to_string();
        }
        rest.to_string()
    } else {
        s.to_string()
    }
}

// Retorna true se o slot corresponde a um temporário interno do compilador (%tN).
fn is_render_temp(slot: &str) -> bool {
    let Some(suffix) = slot.strip_prefix("%t") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
}

// Retorna anotação de papel para rótulos de blocos conhecidos.
// Ajuda humanos a entender o propósito de cada bloco sem alterar semântica.
fn block_role_annotation(label: &str) -> &'static str {
    if label == "entry" {
        return "  ; entrada da função";
    }
    if label.starts_with("then_") {
        return "  ; ramo 'verdadeiro' (talvez)";
    }
    if label.starts_with("else_") {
        return "  ; ramo 'senão'";
    }
    if label.starts_with("loop_cond_") {
        return "  ; condição do loop (sempre que)";
    }
    if label.starts_with("loop_join_") {
        return "  ; saída do loop";
    }
    if label.starts_with("loop_break_cont_") {
        return "  ; caminho auxiliar após quebrar";
    }
    if label.starts_with("loop_continue_cont_") {
        return "  ; caminho auxiliar após continuar";
    }
    if label.starts_with("loop_") {
        return "  ; corpo do loop";
    }
    if label.starts_with("join_") {
        return "  ; ponto de retomada após if/senão";
    }
    if label.starts_with("logic_rhs_") {
        return "  ; avalia lado direito (&&/||)";
    }
    if label.starts_with("logic_short_") {
        return "  ; atalho (curto-circuito)";
    }
    if label.starts_with("logic_join_") {
        return "  ; ponto de continuação após decisão lógica";
    }
    ""
}
// @pinker-nav:end machine.renderizacao.apresentacao

// @pinker-nav:start machine.renderizacao.componentes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer machine
// @pinker-nav:summary Formatação textual de instruções e fluxo da máquina: `render_instr`, `render_term` (com `jmp_comment`/`br_true_comment`/`with_comment`), `render_operand` e o utilitário `line`. Os comentários de fluxo são heurísticos, derivados dos prefixos dos labels — não são metadados semânticos persistidos no modelo. Não altera a máquina, não valida nem executa.
fn render_instr(i: &MachineInstr) -> String {
    match i {
        MachineInstr::PushInt(v) => {
            with_comment(format!("push_int {}", v), "empilha literal inteiro")
        }
        MachineInstr::PushBool(v) => with_comment(
            format!("push_bool {}", if *v { "verdade" } else { "falso" }),
            "empilha literal lógico",
        ),
        MachineInstr::PushStr(v) => {
            with_comment(format!("push_str \"{}\"", v), "empilha literal verso")
        }
        MachineInstr::LoadSlot(s) => with_comment(
            format!("load_slot {}", clean_slot_display(s)),
            "carrega valor do slot para a pilha",
        ),
        MachineInstr::LoadGlobal(g) => with_comment(
            format!("load_global @{}", g),
            "carrega constante global para a pilha",
        ),
        MachineInstr::StoreSlot(s) => {
            let slot = clean_slot_display(s);
            let comment = if is_render_temp(s) {
                format!("guarda o resultado no temporário {}", slot)
            } else {
                format!("atualiza a variável local {}", slot)
            };
            with_comment(format!("store_slot {}", slot), &comment)
        }
        MachineInstr::Neg { ty } => {
            with_comment(format!("neg<{}>", ty.name()), "negação aritmética do topo")
        }
        MachineInstr::Not => with_comment("not".to_string(), "negação lógica do topo"),
        MachineInstr::BitNot { ty } => {
            with_comment(format!("bitnot<{}>", ty.name()), "negação bitwise do topo")
        }
        MachineInstr::DerefLoad { ty, is_volatile } => with_comment(
            format!(
                "{} {}",
                if *is_volatile {
                    "deref_load_fragil"
                } else {
                    "deref_load"
                },
                ty.name()
            ),
            "lê valor indireto a partir de ponteiro no topo",
        ),
        MachineInstr::DerefStore { ty, is_volatile } => with_comment(
            format!(
                "{} {}",
                if *is_volatile {
                    "deref_store_fragil"
                } else {
                    "deref_store"
                },
                ty.name()
            ),
            "escreve valor indireto no endereço apontado",
        ),
        MachineInstr::Cast { ty } => with_comment(
            format!("cast {}", ty.name()),
            "converte valor explícito para o tipo alvo",
        ),
        MachineInstr::MakeUnion {
            union_type_id, tag, ..
        } => format!("make_union #{} tag={}", union_type_id.0, tag),
        MachineInstr::UnionTag { union_type_id } => with_comment(
            format!("union_tag #{}", union_type_id.0),
            "lê a tag corrente da união no topo",
        ),
        MachineInstr::UnionExtract {
            union_type_id,
            tag,
            canonical_member_key,
            ..
        } => with_comment(
            format!(
                "union_extract #{} tag={} key={}",
                union_type_id.0, tag, canonical_member_key
            ),
            "abre o payload do membro já validado",
        ),
        MachineInstr::BitAnd { ty } => {
            with_comment(format!("bitand<{}>", ty.name()), "AND bit a bit entre dois topos")
        }
        MachineInstr::BitOr { ty } => {
            with_comment(format!("bitor<{}>", ty.name()), "OR bit a bit entre dois topos")
        }
        MachineInstr::BitXor { ty } => {
            with_comment(format!("bitxor<{}>", ty.name()), "XOR bit a bit entre dois topos")
        }
        MachineInstr::Shl { ty } => {
            with_comment(format!("shl<{}>", ty.name()), "desloca bits à esquerda")
        }
        MachineInstr::Shr { ty } => {
            with_comment(format!("shr<{}>", ty.name()), "desloca bits à direita")
        }
        MachineInstr::Add { ty } => {
            with_comment(format!("add<{}>", ty.name()), "soma os dois topos da pilha")
        }
        MachineInstr::Sub { ty } => {
            with_comment(format!("sub<{}>", ty.name()), "subtrai os dois topos da pilha")
        }
        MachineInstr::Mul { ty } => {
            with_comment(format!("mul<{}>", ty.name()), "multiplica os dois topos da pilha")
        }
        MachineInstr::Div { ty } => {
            with_comment(format!("div<{}>", ty.name()), "divide os dois topos da pilha")
        }
        MachineInstr::Mod { ty } => {
            with_comment(format!("mod<{}>", ty.name()), "resto da divisão entre dois topos")
        }
        MachineInstr::CmpEq { ty } => {
            with_comment(format!("cmp_eq<{}>", ty.name()), "compara igualdade")
        }
        MachineInstr::CmpNe { ty } => {
            with_comment(format!("cmp_ne<{}>", ty.name()), "compara diferença")
        }
        MachineInstr::CmpLt { ty } => {
            with_comment(format!("cmp_lt<{}>", ty.name()), "compara menor que")
        }
        MachineInstr::CmpLe { ty } => {
            with_comment(format!("cmp_le<{}>", ty.name()), "compara menor ou igual")
        }
        MachineInstr::CmpGt { ty } => {
            with_comment(format!("cmp_gt<{}>", ty.name()), "compara maior que")
        }
        MachineInstr::CmpGe { ty } => {
            with_comment(format!("cmp_ge<{}>", ty.name()), "compara maior ou igual")
        }
        MachineInstr::Call { callee, argc } => with_comment(
            format!("call {}, {}", callee, argc),
            &format!(
                "chama {} com {} argumento(s) e empilha o retorno",
                callee, argc
            ),
        ),
        MachineInstr::CallVoid { callee, argc } => with_comment(
            format!("call_void {}, {}", callee, argc),
            &format!("chama {} com {} argumento(s) sem retorno", callee, argc),
        ),
        MachineInstr::PushFunctionRef(name) => with_comment(
            format!("push_function_ref {}", name),
            "empilha handle callable (descritor estático) da função",
        ),
        MachineInstr::PushRawFunctionRef(name) => with_comment(
            format!("push_raw_function_ref {}", name),
            "empilha endereço cru do símbolo de código",
        ),
        MachineInstr::CallIndirect { argc } => with_comment(
            format!("call_indirect {}", argc),
            &format!(
                "consome handle callable no topo e {} argumento(s) abaixo, empilha o retorno",
                argc
            ),
        ),
        MachineInstr::CallRaw { argc, has_return } => with_comment(
            format!("call_raw {}, {}", argc, has_return),
            &format!(
                "consome endereço cru e {} argumento(s), retorno={}",
                argc, has_return
            ),
        ),
        MachineInstr::MakeClosure {
            function_name,
            capture_count,
        } => with_comment(
            format!("make_closure {}, {}", function_name, capture_count),
            &format!(
                "consome {} valor(es) capturado(s), aloca ambiente e empilha novo handle callable de {}",
                capture_count, function_name
            ),
        ),
        MachineInstr::MakeTraitObject {
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => with_comment(
            format!(
                "make_trait_object trato<{}>, concrete={}:{}, size={}, vtable=[{}]",
                trait_name,
                concrete_type_name,
                concrete_type.name(),
                concrete_size,
                vtable_methods.join(", ")
            ),
            "consome valor concreto e empilha handle de objeto de trato",
        ),
        MachineInstr::TraitCall {
            trait_name,
            method_name,
            method_slot,
            method_count,
            argc,
            param_types: _,
            ret_type,
        } => with_comment(
            format!(
                "trait_call trato<{}>.{}#{}/{}, argc={}, ret={}",
                trait_name,
                method_name,
                method_slot,
                method_count,
                argc,
                ret_type.name()
            ),
            if *ret_type == TypeIR::Nulo {
                "consome objeto e argumentos, despacha pela vtable sem retorno"
            } else {
                "consome objeto e argumentos, despacha pela vtable e empilha o retorno"
            },
        ),
        MachineInstr::PrintIntInline => {
            with_comment("print_int_inline".to_string(), "imprime inteiro sem quebra")
        }
        MachineInstr::PrintBoolInline => {
            with_comment("print_bool_inline".to_string(), "imprime lógico sem quebra")
        }
        MachineInstr::PrintStrValueInline => with_comment(
            "print_str_value_inline".to_string(),
            "imprime verso do topo sem quebra",
        ),
        MachineInstr::PrintStrInline(s) => with_comment(
            format!("print_str_inline \"{}\"", s),
            "imprime literal verso sem quebra",
        ),
        MachineInstr::PrintSpace => with_comment("print_space".to_string(), "imprime espaço"),
        MachineInstr::PrintNewline => with_comment("print_newline".to_string(), "imprime quebra"),
        MachineInstr::InlineAsm { chunks, .. } => format!("inline_asm {:?}", chunks),
    }
}

fn render_term(current_label: &str, t: &MachineTerminator) -> String {
    match t {
        MachineTerminator::Jmp(l) => {
            let comment = jmp_comment(current_label, l);
            with_comment(format!("jmp {}", l), comment)
        }
        MachineTerminator::BrTrue {
            then_label,
            else_label,
        } => {
            let comment = br_true_comment(current_label, then_label, else_label);
            with_comment(format!("br_true {}, {}", then_label, else_label), comment)
        }
        MachineTerminator::Ret => with_comment("ret".to_string(), "retorna o valor atual da pilha"),
        MachineTerminator::RetVoid => {
            with_comment("ret_void".to_string(), "encerra a função sem retorno")
        }
    }
}

fn jmp_comment<'a>(current_label: &'a str, target: &'a str) -> &'a str {
    if target.starts_with("loop_cond_") {
        return "volta para a condição do loop";
    }
    if target.starts_with("loop_join_") {
        return "segue para a saída do loop";
    }
    if target.starts_with("loop_break_cont_") {
        return "segue pelo caminho auxiliar após quebrar";
    }
    if target.starts_with("loop_continue_cont_") {
        return "segue pelo caminho auxiliar após continuar";
    }
    if target.starts_with("join_") {
        return "segue para a convergência dos ramos";
    }
    if target.starts_with("logic_join_") {
        return "continua após o atalho lógico";
    }
    if current_label.starts_with("join_") {
        return "retoma o fluxo após convergência de ramos";
    }
    if current_label.starts_with("logic_join_") {
        return "retoma o fluxo após decisão lógica";
    }
    "salto incondicional para o próximo bloco"
}

fn br_true_comment<'a>(
    current_label: &'a str,
    then_label: &'a str,
    else_label: &'a str,
) -> &'a str {
    if current_label.starts_with("loop_cond_")
        && then_label.starts_with("loop_")
        && else_label.starts_with("loop_join_")
    {
        return "se a condição do loop continuar verdadeira, entra no corpo; senão sai do loop";
    }
    if then_label.starts_with("loop_cond_") && else_label.starts_with("loop_continue_cont_") {
        return "se for para continuar, volta ao teste do loop; senão segue pelo caminho auxiliar";
    }
    if then_label.starts_with("loop_join_") && else_label.starts_with("loop_break_cont_") {
        return "se for para quebrar, sai do loop; senão segue pelo caminho auxiliar";
    }
    if then_label.starts_with("then_") && else_label.starts_with("else_") {
        return "se a condição for verdadeira, entra no ramo 'talvez'; senão vai para o 'senão'";
    }
    if then_label.starts_with("logic_rhs_") && else_label.starts_with("logic_short_") {
        return "se o valor atual ainda não decide o resultado, avalia o lado direito; senão segue pelo atalho lógico";
    }
    if then_label.starts_with("logic_short_") && else_label.starts_with("logic_rhs_") {
        return "se o valor atual já decide o resultado, segue pelo atalho lógico; senão avalia o lado direito";
    }
    "se topo for verdadeiro vai para o primeiro alvo; senão para o segundo"
}

fn with_comment(op: String, comment: &str) -> String {
    format!("{op}  ; {comment}")
}

fn render_operand(op: &OperandIR) -> String {
    match op {
        OperandIR::Int(v) => v.to_string(),
        OperandIR::Bool(v) => {
            if *v {
                "verdade".to_string()
            } else {
                "falso".to_string()
            }
        }
        OperandIR::Str(s) => format!("\"{}\"", s),
        OperandIR::Local(s) => s.clone(),
        OperandIR::GlobalConst(g) => format!("@{}", g),
        OperandIR::Temp(t) => format!("%t{}", t.0),
        OperandIR::FunctionRef(name) => format!("fnref({})", name),
        OperandIR::RawFunctionRef(name) => format!("raw_fnref({})", name),
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
// @pinker-nav:end machine.renderizacao.componentes
