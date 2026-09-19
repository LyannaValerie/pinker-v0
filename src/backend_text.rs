// @pinker-nav:start backend-text.model.representation
// @pinker-nav:domain model
// @pinker-nav:layer backend-text
// @pinker-nav:summary Model of the textual backend: `BackendTextProgram` (module name, `is_freestanding`, globals and functions), `BackendTextFunction` (return type, parameters and locals as slot names — without corresponding types), `BackendTextBlock`, `BackendTextInstruction` (`Mov`/`Unary`/`Binary`/`Call`/`Falar`), `BackendTextFalarArg` and `BackendTextTerminator`. It represents textual operations by reusing `OperandIR`/`TempIR`/`TypeIR`/`UnaryOpIR`/`BinaryOpIR`; it defines neither physical registers, nor a native stack frame, nor an ABI.
use crate::cfg_ir::{FalarArgCfgIR, InstructionCfgIR, OperandIR, ProgramCfgIR, TerminatorIR};
use crate::error::PinkerError;
use crate::instr_select::{FalarArgSelected, SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextProgram {
    pub module_name: String,
    pub is_freestanding: bool,
    pub globals: Vec<BackendTextGlobal>,
    pub functions: Vec<BackendTextFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextGlobal {
    pub name: String,
    pub value: OperandIR,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextFunction {
    pub name: String,
    pub ret_type: TypeIR,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub slot_types: HashMap<String, TypeIR>,
    pub blocks: Vec<BackendTextBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextBlock {
    pub label: String,
    pub instructions: Vec<BackendTextInstruction>,
    pub terminator: BackendTextTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendTextInstruction {
    Mov {
        dest: String,
        src: OperandIR,
    },
    Unary {
        dest: crate::cfg_ir::TempIR,
        op: UnaryOpIR,
        operand: OperandIR,
    },
    Binary {
        dest: crate::cfg_ir::TempIR,
        op: BinaryOpIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    PointerOffset {
        dest: crate::cfg_ir::TempIR,
        pointer: OperandIR,
        offset: OperandIR,
        element_size: u64,
        element_align: u64,
    },
    Call {
        dest: Option<crate::cfg_ir::TempIR>,
        callee: String,
        args: Vec<OperandIR>,
        ret_type: TypeIR,
    },
    CallRaw {
        dest: Option<crate::cfg_ir::TempIR>,
        callee: OperandIR,
        args: Vec<OperandIR>,
        param_types: Vec<TypeIR>,
        ret_type: TypeIR,
    },
    MakeTraitObject {
        dest: crate::cfg_ir::TempIR,
        value: OperandIR,
        trait_name: String,
        concrete_type: TypeIR,
        concrete_type_name: String,
        concrete_size: u64,
        vtable_methods: Vec<String>,
    },
    TraitCall {
        dest: Option<crate::cfg_ir::TempIR>,
        object: OperandIR,
        trait_name: String,
        method_name: String,
        method_slot: u64,
        method_count: u64,
        args: Vec<OperandIR>,
        param_types: Vec<TypeIR>,
        ret_type: TypeIR,
    },
    Falar {
        args: Vec<BackendTextFalarArg>,
    },
    InlineAsm {
        chunks: Vec<String>,
        operands: Vec<crate::cfg_ir::InlineAsmOperandCfgIR>,
        clobbers: Vec<crate::inline_asm::AsmClobber>,
    },
    UnionInject {
        dest: crate::cfg_ir::TempIR,
        value: OperandIR,
        union_type_id: crate::ir::UnionTypeId,
        tag: u64,
    },
    // Operações internas tipadas de união no backend textual. O símbolo de
    // runtime correspondente é escolhido apenas no backend nativo.
    UnionTag {
        dest: crate::cfg_ir::TempIR,
        value: OperandIR,
        union_type_id: crate::ir::UnionTypeId,
    },
    UnionExtract {
        dest: crate::cfg_ir::TempIR,
        value: OperandIR,
        union_type_id: crate::ir::UnionTypeId,
        tag: u64,
        canonical_member_key: String,
        payload_type: TypeIR,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextFalarArg {
    pub value: OperandIR,
    pub ty: TypeIR,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendTextTerminator {
    Jump(String),
    Branch {
        cond: OperandIR,
        then_label: String,
        else_label: String,
    },
    Return(Option<OperandIR>),
}
// @pinker-nav:end backend-text.model.representation

// @pinker-nav:start backend-text.lowering.cfg-program
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Direct lowering from `ProgramCfgIR` to `BackendTextProgram`: it copies globals, functions (parameters/locals as slot names), blocks, instructions and terminators. `DerefLoad` becomes `Unary`/`Deref` discarding `ty` and volatility; `DerefStore` and `Cast` are refused with a synthetic span error `(1,1)`. It is a public path with no callers in the tree (the real pipeline uses `lower_selected_program`); it neither executes the program nor emits native code.
pub fn lower_program(program: &ProgramCfgIR) -> Result<BackendTextProgram, PinkerError> {
    let globals = program
        .consts
        .iter()
        .map(|g| BackendTextGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = program
        .functions
        .iter()
        .map(|f| -> Result<BackendTextFunction, PinkerError> {
            let blocks = f
                .blocks
                .iter()
                .map(|b| -> Result<BackendTextBlock, PinkerError> {
                    let instructions = b
                        .instructions
                        .iter()
                        .map(|i| match i {
                            InstructionCfgIR::Let { slot, value }
                            | InstructionCfgIR::Assign { slot, value } => {
                                Ok(BackendTextInstruction::Mov {
                                    dest: slot.clone(),
                                    src: value.clone(),
                                })
                            }
                            InstructionCfgIR::Unary {
                                dest, op, operand, ..
                            } => Ok(BackendTextInstruction::Unary {
                                dest: *dest,
                                op: *op,
                                operand: operand.clone(),
                            }),
                            InstructionCfgIR::DerefLoad { dest, ptr, .. } => {
                                Ok(BackendTextInstruction::Unary {
                                    dest: *dest,
                                    op: UnaryOpIR::Deref,
                                    operand: ptr.clone(),
                                })
                            }
                            InstructionCfgIR::DerefStore { .. } => Err(PinkerError::Ir {
                                msg: "backend textual ainda não lowera escrita indireta nesta fase"
                                    .to_string(),
                                span: crate::token::Span::single(crate::token::Position::new(1, 1)),
                            }),
                            InstructionCfgIR::Cast { .. } => Err(PinkerError::Ir {
                                msg: "backend textual ainda não lowera cast nesta fase".to_string(),
                                span: crate::token::Span::single(crate::token::Position::new(1, 1)),
                            }),
                            InstructionCfgIR::UnionInject {
                                dest,
                                value,
                                union_type_id,
                                tag,
                                ..
                            } => Ok(BackendTextInstruction::UnionInject {
                                dest: *dest,
                                value: value.clone(),
                                union_type_id: *union_type_id,
                                tag: *tag,
                            }),
                            InstructionCfgIR::UnionTag {
                                dest,
                                value,
                                union_type_id,
                            } => Ok(BackendTextInstruction::UnionTag {
                                dest: *dest,
                                value: value.clone(),
                                union_type_id: *union_type_id,
                            }),
                            InstructionCfgIR::UnionExtract {
                                dest,
                                value,
                                union_type_id,
                                tag,
                                canonical_member_key,
                                payload_type,
                                ..
                            } => Ok(BackendTextInstruction::UnionExtract {
                                dest: *dest,
                                value: value.clone(),
                                union_type_id: *union_type_id,
                                tag: *tag,
                                canonical_member_key: canonical_member_key.clone(),
                                payload_type: *payload_type,
                            }),
                            InstructionCfgIR::Binary {
                                dest, op, lhs, rhs, ..
                            } => Ok(BackendTextInstruction::Binary {
                                dest: *dest,
                                op: *op,
                                lhs: lhs.clone(),
                                rhs: rhs.clone(),
                            }),
                            InstructionCfgIR::PointerOffset {
                                dest,
                                pointer,
                                offset,
                                element_size,
                                element_align,
                                ..
                            } => Ok(BackendTextInstruction::PointerOffset {
                                dest: *dest,
                                pointer: pointer.clone(),
                                offset: offset.clone(),
                                element_size: *element_size,
                                element_align: *element_align,
                            }),
                            InstructionCfgIR::Call {
                                dest,
                                callee,
                                args,
                                ret_type,
                                // #532: o backend textual não despacha
                                // intrínseca por nome — ele emite a chamada
                                // pelo símbolo —, então não há decisão de
                                // identidade a consumir aqui.
                                identidade: _,
                            } => Ok(BackendTextInstruction::Call {
                                dest: *dest,
                                callee: callee.clone(),
                                args: args.clone(),
                                ret_type: *ret_type,
                            }),
                            InstructionCfgIR::CallIndirect { .. } => Err(PinkerError::Ir {
                                msg: "backend textual ainda não lowera chamada indireta (fase 242)"
                                    .to_string(),
                                span: crate::token::Span::single(crate::token::Position::new(1, 1)),
                            }),
                            InstructionCfgIR::CallRaw {
                                dest,
                                callee,
                                args,
                                param_types,
                                ret_type,
                            } => Ok(BackendTextInstruction::CallRaw {
                                dest: *dest,
                                callee: callee.clone(),
                                args: args.clone(),
                                param_types: param_types.clone(),
                                ret_type: *ret_type,
                            }),
                            InstructionCfgIR::MakeClosure { .. } => Err(PinkerError::Ir {
                                msg:
                                    "backend textual ainda não lowera criação de closure (fase 243)"
                                        .to_string(),
                                span: crate::token::Span::single(crate::token::Position::new(1, 1)),
                            }),
                            InstructionCfgIR::MakeTraitObject {
                                dest,
                                value,
                                trait_name,
                                concrete_type,
                                concrete_type_name,
                                concrete_size,
                                vtable_methods,
                            } => Ok(BackendTextInstruction::MakeTraitObject {
                                dest: *dest,
                                value: value.clone(),
                                trait_name: trait_name.clone(),
                                concrete_type: *concrete_type,
                                concrete_type_name: concrete_type_name.clone(),
                                concrete_size: *concrete_size,
                                vtable_methods: vtable_methods.clone(),
                            }),
                            InstructionCfgIR::TraitCall {
                                dest,
                                object,
                                trait_name,
                                method_name,
                                method_slot,
                                method_count,
                                args,
                                param_types,
                                ret_type,
                            } => Ok(BackendTextInstruction::TraitCall {
                                dest: *dest,
                                object: object.clone(),
                                trait_name: trait_name.clone(),
                                method_name: method_name.clone(),
                                method_slot: *method_slot,
                                method_count: *method_count,
                                args: args.clone(),
                                param_types: param_types.clone(),
                                ret_type: *ret_type,
                            }),
                            InstructionCfgIR::Falar { args } => Ok(BackendTextInstruction::Falar {
                                args: map_falar_args_from_cfg(args),
                            }),
                            InstructionCfgIR::InlineAsm {
                                chunks,
                                operands,
                                clobbers,
                                ..
                            } => Ok(BackendTextInstruction::InlineAsm {
                                chunks: chunks.clone(),
                                operands: operands.clone(),
                                clobbers: clobbers.clone(),
                            }),
                        })
                        .collect::<Result<Vec<_>, PinkerError>>()?;
                    Ok(BackendTextBlock {
                        label: b.label.clone(),
                        instructions,
                        terminator: match &b.terminator {
                            TerminatorIR::Jump(label) => BackendTextTerminator::Jump(label.clone()),
                            TerminatorIR::Branch {
                                cond,
                                then_label,
                                else_label,
                            } => BackendTextTerminator::Branch {
                                cond: cond.clone(),
                                then_label: then_label.clone(),
                                else_label: else_label.clone(),
                            },
                            TerminatorIR::Return(v) => BackendTextTerminator::Return(v.clone()),
                        },
                    })
                })
                .collect::<Result<Vec<_>, PinkerError>>()?;
            Ok(BackendTextFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.iter().map(|p| p.slot.clone()).collect(),
                locals: f.locals.iter().map(|l| l.slot.clone()).collect(),
                slot_types: f
                    .params
                    .iter()
                    .map(|p| (p.slot.clone(), p.ty))
                    .chain(f.locals.iter().map(|l| (l.slot.clone(), l.ty)))
                    .collect(),
                blocks,
            })
        })
        .collect::<Result<Vec<_>, PinkerError>>()?;

    Ok(BackendTextProgram {
        module_name: program.module_name.clone(),
        is_freestanding: program.is_freestanding,
        globals,
        functions,
    })
}
// @pinker-nav:end backend-text.lowering.cfg-program

// @pinker-nav:start backend-text.lowering.program-selection
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Lowering from `SelectedProgram` to `BackendTextProgram` — the path actually used (by `emit_program`, by the `--backend-text` CLI and by `backend_s`). It copies globals and functions (parameters/locals as slot names, without types), delegating each instruction to `map_selected_instr` and each terminator to `map_selected_term`, and preserves the module and `is_freestanding`.
pub fn lower_selected_program(
    selected: &SelectedProgram,
) -> Result<BackendTextProgram, PinkerError> {
    let globals = selected
        .globals
        .iter()
        .map(|g| BackendTextGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = selected
        .functions
        .iter()
        .map(|f| -> Result<BackendTextFunction, PinkerError> {
            let blocks = f
                .blocks
                .iter()
                .map(|b| -> Result<BackendTextBlock, PinkerError> {
                    Ok(BackendTextBlock {
                        label: b.label.clone(),
                        instructions: b
                            .instructions
                            .iter()
                            .map(map_selected_instr)
                            .collect::<Result<Vec<_>, PinkerError>>()?,
                        terminator: map_selected_term(&b.terminator),
                    })
                })
                .collect::<Result<Vec<_>, PinkerError>>()?;
            Ok(BackendTextFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.clone(),
                locals: f.locals.clone(),
                slot_types: f.slot_types.clone(),
                blocks,
            })
        })
        .collect::<Result<Vec<_>, PinkerError>>()?;

    Ok(BackendTextProgram {
        module_name: selected.module_name.clone(),
        is_freestanding: selected.is_freestanding,
        globals,
        functions,
    })
}
// @pinker-nav:end backend-text.lowering.program-selection

// @pinker-nav:start backend-text.lowering.selected-instructions
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Mapping of the selected instructions and terminators into the textual backend's generic representation: `map_selected_instr` reconverts each `SelectedInstr` (`Mov`, unaries, arithmetic/bitwise/shift/comparison via `UnaryOpIR`/`BinaryOpIR`, `Call`/`CallVoid`, `Falar`) and `map_selected_term` translates `Jmp`/`Br`/`Ret`. `DerefLoad` becomes `Unary`/`Deref` (discarding volatility); `DerefStore` and `Cast` are refused with a synthetic span `(1,1)`; `CallVoid` becomes `Call` with `dest` absent and a `Nulo` return. `map_selected_term` is folded in here because it is a trivial adjacent mapping.
fn map_selected_instr(i: &SelectedInstr) -> Result<BackendTextInstruction, PinkerError> {
    match i {
        SelectedInstr::Mov { dest, src } => Ok(BackendTextInstruction::Mov {
            dest: dest.clone(),
            src: src.clone(),
        }),
        SelectedInstr::Neg { dest, operand, .. } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::Neg,
            operand: operand.clone(),
        }),
        SelectedInstr::Not { dest, operand } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::Not,
            operand: operand.clone(),
        }),
        SelectedInstr::BitNot { dest, operand, .. } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::BitNot,
            operand: operand.clone(),
        }),
        SelectedInstr::DerefLoad { dest, ptr, .. } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::Deref,
            operand: ptr.clone(),
        }),
        SelectedInstr::DerefStore { .. } => Err(PinkerError::Ir {
            msg: "backend textual ainda não lowera escrita indireta nesta fase".to_string(),
            span: crate::token::Span::single(crate::token::Position::new(1, 1)),
        }),
        SelectedInstr::Cast { .. } => Err(PinkerError::Ir {
            msg: "backend textual ainda não lowera cast nesta fase".to_string(),
            span: crate::token::Span::single(crate::token::Position::new(1, 1)),
        }),
        SelectedInstr::UnionInject {
            dest,
            value,
            union_type_id,
            tag,
            ..
        } => Ok(BackendTextInstruction::UnionInject {
            dest: *dest,
            value: value.clone(),
            union_type_id: *union_type_id,
            tag: *tag,
        }),
        SelectedInstr::UnionTag {
            dest,
            value,
            union_type_id,
        } => Ok(BackendTextInstruction::UnionTag {
            dest: *dest,
            value: value.clone(),
            union_type_id: *union_type_id,
        }),
        SelectedInstr::UnionExtract {
            dest,
            value,
            union_type_id,
            tag,
            canonical_member_key,
            payload_type,
            ..
        } => Ok(BackendTextInstruction::UnionExtract {
            dest: *dest,
            value: value.clone(),
            union_type_id: *union_type_id,
            tag: *tag,
            canonical_member_key: canonical_member_key.clone(),
            payload_type: *payload_type,
        }),
        SelectedInstr::BitAnd { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitAnd,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::BitOr { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitOr,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::BitXor { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitXor,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Shl { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Shl,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Shr { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Shr,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Add { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Add,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::PointerOffset {
            dest,
            pointer,
            offset,
            element_size,
            element_align,
            ..
        } => Ok(BackendTextInstruction::PointerOffset {
            dest: *dest,
            pointer: pointer.clone(),
            offset: offset.clone(),
            element_size: *element_size,
            element_align: *element_align,
        }),
        SelectedInstr::Sub { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Sub,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Mul { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Mul,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Div { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Div,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Mod { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Mod,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpEq { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Eq,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpNe { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Neq,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpLt { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Lt,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpLe { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Lte,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpGt { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Gt,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpGe { dest, lhs, rhs, .. } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Gte,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Call {
            dest,
            callee,
            args,
            ret_type,
            identidade: _,
        } => Ok(BackendTextInstruction::Call {
            dest: Some(*dest),
            callee: callee.clone(),
            args: args.clone(),
            ret_type: *ret_type,
        }),
        SelectedInstr::CallVoid {
            callee,
            args,
            identidade: _,
        } => Ok(BackendTextInstruction::Call {
            dest: None,
            callee: callee.clone(),
            args: args.clone(),
            ret_type: TypeIR::Nulo,
        }),
        SelectedInstr::CallIndirect { .. } => Err(PinkerError::Ir {
            msg: "backend textual ainda não lowera chamada indireta selecionada (fase 242)"
                .to_string(),
            span: crate::token::Span::single(crate::token::Position::new(1, 1)),
        }),
        SelectedInstr::CallRaw {
            dest,
            callee,
            args,
            param_types,
            ret_type,
        } => Ok(BackendTextInstruction::CallRaw {
            dest: *dest,
            callee: callee.clone(),
            args: args.clone(),
            param_types: param_types.clone(),
            ret_type: *ret_type,
        }),
        SelectedInstr::MakeClosure { .. } => Err(PinkerError::Ir {
            msg: "backend textual ainda não lowera criação de closure (fase 243)".to_string(),
            span: crate::token::Span::single(crate::token::Position::new(1, 1)),
        }),
        SelectedInstr::MakeTraitObject {
            dest,
            value,
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => Ok(BackendTextInstruction::MakeTraitObject {
            dest: *dest,
            value: value.clone(),
            trait_name: trait_name.clone(),
            concrete_type: *concrete_type,
            concrete_type_name: concrete_type_name.clone(),
            concrete_size: *concrete_size,
            vtable_methods: vtable_methods.clone(),
        }),
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
        } => Ok(BackendTextInstruction::TraitCall {
            dest: *dest,
            object: object.clone(),
            trait_name: trait_name.clone(),
            method_name: method_name.clone(),
            method_slot: *method_slot,
            method_count: *method_count,
            args: args.clone(),
            param_types: param_types.clone(),
            ret_type: *ret_type,
        }),
        SelectedInstr::Falar { args } => Ok(BackendTextInstruction::Falar {
            args: map_falar_args_from_selected(args),
        }),
        SelectedInstr::InlineAsm {
            chunks,
            operands,
            clobbers,
            ..
        } => Ok(BackendTextInstruction::InlineAsm {
            chunks: chunks.clone(),
            operands: operands.clone(),
            clobbers: clobbers.clone(),
        }),
    }
}

fn map_selected_term(t: &SelectedTerminator) -> BackendTextTerminator {
    match t {
        SelectedTerminator::Jmp(l) => BackendTextTerminator::Jump(l.clone()),
        SelectedTerminator::Br {
            cond,
            then_label,
            else_label,
        } => BackendTextTerminator::Branch {
            cond: cond.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        },
        SelectedTerminator::Ret(v) => BackendTextTerminator::Return(v.clone()),
    }
}
// @pinker-nav:end backend-text.lowering.selected-instructions

// @pinker-nav:start backend-text.pipeline.emission
// @pinker-nav:domain pipeline
// @pinker-nav:layer backend-text
// @pinker-nav:summary `emit_program`: public pipeline that chains `ProgramCfgIR` → `instr_select::lower_program` → `instr_select_validate::validate_program` → `lower_selected_program` → `backend_text_validate::validate_program` → `render_program`, returning the validated textual pseudo-assembly. It is not native compilation; it has no callers in the tree (the CLI interleaves the same steps).
pub fn emit_program(program: &ProgramCfgIR) -> Result<String, PinkerError> {
    let selected = crate::instr_select::lower_program(program)?;
    crate::instr_select_validate::validate_program(&selected)?;
    let lowered = lower_selected_program(&selected)?;
    crate::backend_text_validate::validate_program(&lowered)?;
    Ok(render_program(&lowered))
}
// @pinker-nav:end backend-text.pipeline.emission

// @pinker-nav:start backend-text.rendering.program
// @pinker-nav:domain rendering
// @pinker-nav:layer backend-text
// @pinker-nav:summary `render_program`: serializes the `BackendTextProgram` into textual pseudo-assembly — `module`, `mode livre`/`hospedado`, `globals:`, `text:`, and per function `func`/`params`/`locals` and each block (label, `ins` and `term`), delegating to the component renderers. It receives the finished representation; it does not lower again, does not validate and does not emit native code.
pub fn render_program(program: &BackendTextProgram) -> String {
    let mut out = String::new();

    line(&mut out, 0, &format!("module {}", program.module_name));
    line(
        &mut out,
        0,
        &format!(
            "mode {}",
            if program.is_freestanding {
                "livre"
            } else {
                "hospedado"
            }
        ),
    );
    line(&mut out, 0, "globals:");
    if program.globals.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for global in &program.globals {
            line(
                &mut out,
                1,
                &format!(
                    "global @{} = {}",
                    global.name,
                    render_operand(&global.value)
                ),
            );
        }
    }

    line(&mut out, 0, "text:");
    for function in &program.functions {
        line(&mut out, 1, &format!("func {}:", function.name));
        line(
            &mut out,
            2,
            &format!(
                "params {}",
                if function.params.is_empty() {
                    "[]".to_string()
                } else {
                    function.params.join(", ")
                }
            ),
        );
        line(
            &mut out,
            2,
            &format!(
                "locals {}",
                if function.locals.is_empty() {
                    "[]".to_string()
                } else {
                    function.locals.join(", ")
                }
            ),
        );
        for block in &function.blocks {
            line(&mut out, 2, &format!("{}:", block.label));
            for instruction in &block.instructions {
                line(
                    &mut out,
                    3,
                    &format!("ins {}", render_instruction(instruction)),
                );
            }
            line(
                &mut out,
                3,
                &format!("term {}", render_terminator(&block.terminator)),
            );
        }
    }

    out
}
// @pinker-nav:end backend-text.rendering.program

// @pinker-nav:start backend-text.rendering.instructions
// @pinker-nav:domain rendering
// @pinker-nav:layer backend-text
// @pinker-nav:summary `render_instruction`: formats each `BackendTextInstruction` — `mov`, `unop`, `binop`, `call`/`call_void` (with a defensive branch `(dest absent, non-null return)` that prints the destination as `_`, not produced by the mappers) and `falar` (`value:type` pairs). It produces one textual line per instruction; it does not alter the representation.
fn render_instruction(inst: &BackendTextInstruction) -> String {
    match inst {
        BackendTextInstruction::Mov { dest, src } => {
            format!("mov {}, {}", dest, render_operand(src))
        }
        BackendTextInstruction::Unary { dest, op, operand } => {
            format!(
                "unop {}, {}, {}",
                render_temp(*dest),
                op_name(*op),
                render_operand(operand)
            )
        }
        BackendTextInstruction::Binary { dest, op, lhs, rhs } => format!(
            "binop {}, {}, {}, {}",
            render_temp(*dest),
            binop_name(*op),
            render_operand(lhs),
            render_operand(rhs)
        ),
        BackendTextInstruction::PointerOffset {
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
        BackendTextInstruction::Call {
            dest,
            callee,
            args,
            ret_type,
        } => {
            let args = args
                .iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", ");
            match (dest, ret_type) {
                (Some(dest), _) => {
                    format!(
                        "call {}, {}({}), {}",
                        render_temp(*dest),
                        callee,
                        args,
                        ret_type.name()
                    )
                }
                (None, TypeIR::Nulo) => format!("call_void {}({})", callee, args),
                (None, _) => format!("call {}, {}({}), {}", "_", callee, args, ret_type.name()),
            }
        }
        BackendTextInstruction::CallRaw {
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
                Some(dest) => format!("{} = {}", render_temp(*dest), call),
                None => call,
            }
        }
        BackendTextInstruction::MakeTraitObject {
            dest,
            value,
            trait_name,
            concrete_type_name,
            concrete_size,
            vtable_methods,
            ..
        } => format!(
            "make_trait_object %{}, {} as trato<{}> snapshot={}({}) vtable=[{}]",
            dest.0,
            render_operand(value),
            trait_name,
            concrete_size,
            concrete_type_name,
            vtable_methods.join(", ")
        ),
        BackendTextInstruction::TraitCall {
            dest,
            object,
            trait_name,
            method_name,
            method_slot,
            method_count,
            args,
            ret_type,
            ..
        } => {
            let args = args
                .iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", ");
            let call = format!(
                "trait_call {} trato<{}>.{}[{}/{}]({})",
                render_operand(object),
                trait_name,
                method_name,
                method_slot,
                method_count,
                args
            );
            match (dest, ret_type) {
                (Some(dest), _) => format!("%{} = {} -> {}", dest.0, call, ret_type.name()),
                (None, TypeIR::Nulo) => call,
                (None, _) => format!("_ = {} -> {}", call, ret_type.name()),
            }
        }
        BackendTextInstruction::Falar { args } => format!(
            "falar {}",
            args.iter()
                .map(|arg| format!("{}:{}", render_operand(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        BackendTextInstruction::InlineAsm {
            chunks,
            operands,
            clobbers,
        } => format!(
            "inline_asm {:?} operands={} clobbers={:?}",
            chunks,
            operands.len(),
            clobbers
        ),
        BackendTextInstruction::UnionInject {
            dest,
            value,
            union_type_id,
            tag,
        } => format!(
            "%{} = union_inject #{} tag={} {}",
            dest.0,
            union_type_id.0,
            tag,
            render_operand(value)
        ),
        BackendTextInstruction::UnionTag {
            dest,
            value,
            union_type_id,
        } => format!(
            "%{} = union_tag #{} {}",
            dest.0,
            union_type_id.0,
            render_operand(value)
        ),
        BackendTextInstruction::UnionExtract {
            dest,
            value,
            union_type_id,
            tag,
            canonical_member_key,
            payload_type,
        } => format!(
            "%{} = union_extract #{} tag={} key={} {} -> {}",
            dest.0,
            union_type_id.0,
            tag,
            canonical_member_key,
            render_operand(value),
            payload_type.name()
        ),
    }
}
// @pinker-nav:end backend-text.rendering.instructions

// Ajudantes de lowering de argumentos de `falar` (de CFG e de seleção),
// fisicamente entre os renderizadores; helpers triviais deixados sem âncora.
// @pinker-nav:start backend-text.falar.arguments
// @pinker-nav:domain falar
// @pinker-nav:layer backend-text
// @pinker-nav:summary Projection of `falar` arguments into the textual backend's form from the two origins that reach this point, CFG and instruction selection, preserving the value and the type of each argument without deciding formatting.
fn map_falar_args_from_cfg(args: &[FalarArgCfgIR]) -> Vec<BackendTextFalarArg> {
    args.iter()
        .map(|arg| BackendTextFalarArg {
            value: arg.value.clone(),
            ty: arg.ty,
        })
        .collect()
}

fn map_falar_args_from_selected(args: &[FalarArgSelected]) -> Vec<BackendTextFalarArg> {
    args.iter()
        .map(|arg| BackendTextFalarArg {
            value: arg.value.clone(),
            ty: arg.ty,
        })
        .collect()
}
// @pinker-nav:end backend-text.falar.arguments

// @pinker-nav:start backend-text.rendering.components
// @pinker-nav:domain rendering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Component renderers of the textual backend: `render_terminator` (`jmp`/`br`/`ret`), `render_operand` (locals, `@` globals, integers, `verdade`/`falso`, quoted strings **without escaping** quotes/backslashes/control characters, temporaries), `render_temp` (`%tN`), the operator names `op_name`/`binop_name` and the `line` indentation utility. They serialize individual elements; they do not alter the representation.
fn render_terminator(term: &BackendTextTerminator) -> String {
    match term {
        BackendTextTerminator::Jump(label) => format!("jmp {}", label),
        BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            then_label,
            else_label
        ),
        BackendTextTerminator::Return(Some(value)) => format!("ret {}", render_operand(value)),
        BackendTextTerminator::Return(None) => "ret".to_string(),
    }
}

fn render_operand(operand: &OperandIR) -> String {
    match operand {
        OperandIR::Local(slot) => slot.clone(),
        OperandIR::GlobalConst(name) => format!("@{}", name),
        OperandIR::Int(value) => value.to_string(),
        OperandIR::Bool(value) => {
            if *value {
                "verdade".to_string()
            } else {
                "falso".to_string()
            }
        }
        OperandIR::Str(s) => format!("\"{}\"", s),
        OperandIR::Temp(temp) => render_temp(*temp),
        OperandIR::FunctionRef(name) => format!("fnref({})", name),
        OperandIR::RawFunctionRef(name) => format!("raw_fnref({})", name),
    }
}

fn render_temp(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn op_name(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
        UnaryOpIR::BitNot => "bitnot",
        UnaryOpIR::Deref => "deref",
    }
}

fn binop_name(op: BinaryOpIR) -> &'static str {
    match op {
        BinaryOpIR::LogicalAnd => "and",
        BinaryOpIR::LogicalOr => "or",
        BinaryOpIR::BitAnd => "bitand",
        BinaryOpIR::BitOr => "bitor",
        BinaryOpIR::BitXor => "bitxor",
        BinaryOpIR::Shl => "shl",
        BinaryOpIR::Shr => "shr",
        BinaryOpIR::Add => "add",
        BinaryOpIR::Sub => "sub",
        BinaryOpIR::Mul => "mul",
        BinaryOpIR::Div => "div",
        BinaryOpIR::Mod => "mod",
        BinaryOpIR::Eq => "eq",
        BinaryOpIR::Neq => "neq",
        BinaryOpIR::Lt => "lt",
        BinaryOpIR::Lte => "lte",
        BinaryOpIR::Gt => "gt",
        BinaryOpIR::Gte => "gte",
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
// @pinker-nav:end backend-text.rendering.components
