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
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::TypeIR;
use std::collections::HashMap;

/// Programa completo na representação de máquina abstrata.
/// Contém globals (constantes somente-leitura) e funções com blocos de instruções de pilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgram {
    pub module_name: String,
    pub globals: Vec<MachineGlobal>,
    pub functions: Vec<MachineFunction>,
}

/// Constante global somente-leitura. O runtime acessa via `LoadGlobal`; escrita não é suportada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineGlobal {
    pub name: String,
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
    LoadSlot(String),
    LoadGlobal(String),
    StoreSlot(String),
    Neg,
    Not,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    CmpEq,
    CmpNe,
    CmpLt,
    CmpLe,
    CmpGt,
    CmpGe,
    Call { callee: String, argc: usize },
    CallVoid { callee: String, argc: usize },
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

pub fn lower_program(selected: &SelectedProgram) -> Result<MachineProgram, PinkerError> {
    let globals = selected
        .globals
        .iter()
        .map(|g| MachineGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = selected
        .functions
        .iter()
        .map(|f| {
            let blocks = f
                .blocks
                .iter()
                .map(|b| {
                    let mut code = Vec::new();
                    for i in &b.instructions {
                        lower_instr(i, &mut code);
                    }
                    let terminator = lower_term(&b.terminator, &mut code);
                    MachineBlock {
                        label: b.label.clone(),
                        code,
                        terminator,
                    }
                })
                .collect();

            MachineFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.clone(),
                locals: f.locals.clone(),
                slot_types: f.slot_types.clone(),
                blocks,
            }
        })
        .collect();

    Ok(MachineProgram {
        module_name: selected.module_name.clone(),
        globals,
        functions,
    })
}

// Padrão de lowering para instruções binárias/unárias: carrega operandos na pilha,
// emite a operação, depois armazena o resultado em um slot temporário `%tN`.
fn lower_instr(inst: &SelectedInstr, code: &mut Vec<MachineInstr>) {
    match inst {
        SelectedInstr::Mov { dest, src } => {
            emit_load(src, code);
            code.push(MachineInstr::StoreSlot(dest.clone()));
        }
        SelectedInstr::Neg { dest, operand } => {
            emit_load(operand, code);
            code.push(MachineInstr::Neg);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Not { dest, operand } => {
            emit_load(operand, code);
            code.push(MachineInstr::Not);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitAnd { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitAnd);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitOr { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitOr);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::BitXor { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::BitXor);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Shl { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Shl);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Shr { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Shr);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Add { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Add);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Sub { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Sub);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Mul { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Mul);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Div { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Div);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Mod { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Mod);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpEq { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpEq);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpNe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpNe);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLt { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLt);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLe);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGt { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGt);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGe);
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
    }
}

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

fn emit_load(op: &OperandIR, code: &mut Vec<MachineInstr>) {
    match op {
        OperandIR::Int(v) => code.push(MachineInstr::PushInt(*v)),
        OperandIR::Bool(v) => code.push(MachineInstr::PushBool(*v)),
        OperandIR::Local(s) => code.push(MachineInstr::LoadSlot(s.clone())),
        OperandIR::GlobalConst(g) => code.push(MachineInstr::LoadGlobal(g.clone())),
        OperandIR::Temp(t) => code.push(MachineInstr::LoadSlot(temp_name(*t))),
    }
}

// Temporários recebem o nome canônico `%tN` (N = índice do TempIR).
// Esse padrão é reconhecido pelo validador em `abstract_machine_validate::is_temp_slot`.
fn temp_name(t: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", t.0)
}

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

fn render_instr(i: &MachineInstr) -> String {
    match i {
        MachineInstr::PushInt(v) => {
            with_comment(format!("push_int {}", v), "empilha literal inteiro")
        }
        MachineInstr::PushBool(v) => with_comment(
            format!("push_bool {}", if *v { "verdade" } else { "falso" }),
            "empilha literal lógico",
        ),
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
        MachineInstr::Neg => with_comment("neg".to_string(), "negação aritmética do topo"),
        MachineInstr::Not => with_comment("not".to_string(), "negação lógica do topo"),
        MachineInstr::BitAnd => {
            with_comment("bitand".to_string(), "AND bit a bit entre dois topos")
        }
        MachineInstr::BitOr => with_comment("bitor".to_string(), "OR bit a bit entre dois topos"),
        MachineInstr::BitXor => {
            with_comment("bitxor".to_string(), "XOR bit a bit entre dois topos")
        }
        MachineInstr::Shl => with_comment("shl".to_string(), "desloca bits à esquerda"),
        MachineInstr::Shr => with_comment("shr".to_string(), "desloca bits à direita"),
        MachineInstr::Add => with_comment("add".to_string(), "soma os dois topos da pilha"),
        MachineInstr::Sub => with_comment("sub".to_string(), "subtrai os dois topos da pilha"),
        MachineInstr::Mul => with_comment("mul".to_string(), "multiplica os dois topos da pilha"),
        MachineInstr::Div => with_comment("div".to_string(), "divide os dois topos da pilha"),
        MachineInstr::Mod => with_comment("mod".to_string(), "resto da divisão entre dois topos"),
        MachineInstr::CmpEq => with_comment("cmp_eq".to_string(), "compara igualdade"),
        MachineInstr::CmpNe => with_comment("cmp_ne".to_string(), "compara diferença"),
        MachineInstr::CmpLt => with_comment("cmp_lt".to_string(), "compara menor que"),
        MachineInstr::CmpLe => with_comment("cmp_le".to_string(), "compara menor ou igual"),
        MachineInstr::CmpGt => with_comment("cmp_gt".to_string(), "compara maior que"),
        MachineInstr::CmpGe => with_comment("cmp_ge".to_string(), "compara maior ou igual"),
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
        OperandIR::Local(s) => s.clone(),
        OperandIR::GlobalConst(g) => format!("@{}", g),
        OperandIR::Temp(t) => format!("%t{}", t.0),
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
