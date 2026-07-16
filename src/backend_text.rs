use crate::cfg_ir::{FalarArgCfgIR, InstructionCfgIR, OperandIR, ProgramCfgIR, TerminatorIR};
use crate::error::PinkerError;
use crate::instr_select::{FalarArgSelected, SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};

// @pinker-nav:start backend-text.modelo.representacao
// @pinker-nav:domain modelo
// @pinker-nav:layer backend-text
// @pinker-nav:summary Modelo do backend textual: `BackendTextProgram` (nome de módulo, `is_freestanding`, globais e funções), `BackendTextFunction` (tipo de retorno, parâmetros e locais como nomes de slot — sem tipos correspondentes), `BackendTextBlock`, `BackendTextInstruction` (`Mov`/`Unary`/`Binary`/`Call`/`Falar`), `BackendTextFalarArg` e `BackendTextTerminator`. Representa operações textuais reutilizando `OperandIR`/`TempIR`/`TypeIR`/`UnaryOpIR`/`BinaryOpIR`; não define registradores físicos, stack frame nativo nem ABI.
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
    Call {
        dest: Option<crate::cfg_ir::TempIR>,
        callee: String,
        args: Vec<OperandIR>,
        ret_type: TypeIR,
    },
    Falar {
        args: Vec<BackendTextFalarArg>,
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
// @pinker-nav:end backend-text.modelo.representacao

// @pinker-nav:start backend-text.lowering.cfg-programa
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Lowering direto de `ProgramCfgIR` para `BackendTextProgram`: copia globais, funções (parâmetros/locais como nomes de slot), blocos, instruções e terminadores. `DerefLoad` vira `Unary`/`Deref` descartando `ty` e volatilidade; `DerefStore` e `Cast` são recusados com erro de span sintético `(1,1)`. É um caminho público sem chamadores na árvore (o pipeline real usa `lower_selected_program`); não executa o programa nem emite código nativo.
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
                            InstructionCfgIR::Unary { dest, op, operand } => {
                                Ok(BackendTextInstruction::Unary {
                                    dest: *dest,
                                    op: *op,
                                    operand: operand.clone(),
                                })
                            }
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
                            InstructionCfgIR::Binary { dest, op, lhs, rhs } => {
                                Ok(BackendTextInstruction::Binary {
                                    dest: *dest,
                                    op: *op,
                                    lhs: lhs.clone(),
                                    rhs: rhs.clone(),
                                })
                            }
                            InstructionCfgIR::Call {
                                dest,
                                callee,
                                args,
                                ret_type,
                            } => Ok(BackendTextInstruction::Call {
                                dest: *dest,
                                callee: callee.clone(),
                                args: args.clone(),
                                ret_type: *ret_type,
                            }),
                            InstructionCfgIR::Falar { args } => Ok(BackendTextInstruction::Falar {
                                args: map_falar_args_from_cfg(args),
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
// @pinker-nav:end backend-text.lowering.cfg-programa

// @pinker-nav:start backend-text.lowering.selecao-programa
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Lowering de `SelectedProgram` para `BackendTextProgram` — o caminho efetivamente usado (por `emit_program`, pela CLI `--backend-text` e por `backend_s`). Copia globais e funções (parâmetros/locais como nomes de slot, sem tipos), delegando cada instrução a `map_selected_instr` e cada terminador a `map_selected_term`, e preserva módulo e `is_freestanding`.
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
// @pinker-nav:end backend-text.lowering.selecao-programa

// @pinker-nav:start backend-text.lowering.instrucoes-selecionadas
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-text
// @pinker-nav:summary Mapeamento das instruções e terminadores selecionados para a representação genérica do backend textual: `map_selected_instr` reconverte cada `SelectedInstr` (`Mov`, unários, aritmética/bitwise/shift/comparação via `UnaryOpIR`/`BinaryOpIR`, `Call`/`CallVoid`, `Falar`) e `map_selected_term` traduz `Jmp`/`Br`/`Ret`. `DerefLoad` vira `Unary`/`Deref` (descarta volatilidade); `DerefStore` e `Cast` são recusados com span sintético `(1,1)`; `CallVoid` vira `Call` com `dest` ausente e retorno `Nulo`. `map_selected_term` é dobrado aqui por ser um mapeamento trivial adjacente.
fn map_selected_instr(i: &SelectedInstr) -> Result<BackendTextInstruction, PinkerError> {
    match i {
        SelectedInstr::Mov { dest, src } => Ok(BackendTextInstruction::Mov {
            dest: dest.clone(),
            src: src.clone(),
        }),
        SelectedInstr::Neg { dest, operand } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::Neg,
            operand: operand.clone(),
        }),
        SelectedInstr::Not { dest, operand } => Ok(BackendTextInstruction::Unary {
            dest: *dest,
            op: UnaryOpIR::Not,
            operand: operand.clone(),
        }),
        SelectedInstr::BitNot { dest, operand } => Ok(BackendTextInstruction::Unary {
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
        SelectedInstr::BitAnd { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitAnd,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::BitOr { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitOr,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::BitXor { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::BitXor,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Shl { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Shl,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Shr { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Shr,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Add { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Add,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Sub { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Sub,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Mul { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Mul,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Div { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Div,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::Mod { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Mod,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpEq { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Eq,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpNe { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Neq,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpLt { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Lt,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpLe { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Lte,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpGt { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
            dest: *dest,
            op: BinaryOpIR::Gt,
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        }),
        SelectedInstr::CmpGe { dest, lhs, rhs } => Ok(BackendTextInstruction::Binary {
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
        } => Ok(BackendTextInstruction::Call {
            dest: Some(*dest),
            callee: callee.clone(),
            args: args.clone(),
            ret_type: *ret_type,
        }),
        SelectedInstr::CallVoid { callee, args } => Ok(BackendTextInstruction::Call {
            dest: None,
            callee: callee.clone(),
            args: args.clone(),
            ret_type: TypeIR::Nulo,
        }),
        SelectedInstr::Falar { args } => Ok(BackendTextInstruction::Falar {
            args: map_falar_args_from_selected(args),
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
// @pinker-nav:end backend-text.lowering.instrucoes-selecionadas

// @pinker-nav:start backend-text.pipeline.emissao
// @pinker-nav:domain pipeline
// @pinker-nav:layer backend-text
// @pinker-nav:summary `emit_program`: pipeline público que encadeia `ProgramCfgIR` → `instr_select::lower_program` → `instr_select_validate::validate_program` → `lower_selected_program` → `backend_text_validate::validate_program` → `render_program`, devolvendo o pseudo-assembly textual validado. Não é compilação nativa; não tem chamadores na árvore (a CLI intercala os mesmos passos).
pub fn emit_program(program: &ProgramCfgIR) -> Result<String, PinkerError> {
    let selected = crate::instr_select::lower_program(program)?;
    crate::instr_select_validate::validate_program(&selected)?;
    let lowered = lower_selected_program(&selected)?;
    crate::backend_text_validate::validate_program(&lowered)?;
    Ok(render_program(&lowered))
}
// @pinker-nav:end backend-text.pipeline.emissao

// @pinker-nav:start backend-text.renderizacao.programa
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-text
// @pinker-nav:summary `render_program`: serializa o `BackendTextProgram` em pseudo-assembly textual — `module`, `mode livre`/`hospedado`, `globals:`, `text:`, e por função `func`/`params`/`locals` e cada bloco (rótulo, `ins` e `term`), delegando aos renderizadores de componentes. Recebe a representação pronta; não abaixa de novo, não valida e não emite código nativo.
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
// @pinker-nav:end backend-text.renderizacao.programa

// @pinker-nav:start backend-text.renderizacao.instrucoes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-text
// @pinker-nav:summary `render_instruction`: formata cada `BackendTextInstruction` — `mov`, `unop`, `binop`, `call`/`call_void` (com um ramo defensivo `(dest ausente, retorno não-nulo)` que imprime o destino como `_`, não produzido pelos mapeadores) e `falar` (pares `valor:tipo`). Produz uma linha textual por instrução; não altera a representação.
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
        BackendTextInstruction::Falar { args } => format!(
            "falar {}",
            args.iter()
                .map(|arg| format!("{}:{}", render_operand(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
// @pinker-nav:end backend-text.renderizacao.instrucoes

// Ajudantes de lowering de argumentos de `falar` (de CFG e de seleção),
// fisicamente entre os renderizadores; helpers triviais deixados sem âncora.
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

// @pinker-nav:start backend-text.renderizacao.componentes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-text
// @pinker-nav:summary Renderizadores de componentes do backend textual: `render_terminator` (`jmp`/`br`/`ret`), `render_operand` (locais, globais `@`, inteiros, `verdade`/`falso`, strings entre aspas **sem escape** de aspas/barras/controle, temporários), `render_temp` (`%tN`), os nomes de operadores `op_name`/`binop_name` e o utilitário `line` de indentação. Serializam elementos individuais; não alteram a representação.
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
// @pinker-nav:end backend-text.renderizacao.componentes
