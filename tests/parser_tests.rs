mod common;

use common::{parse, parse_and_check};
use pinker_v0::ast::{ExprKind, Item, Stmt};

#[test]
fn parser_de_funcao_simples() {
    let program = parse("pacote main; carinho principal() -> bombom { mimo 0; }").unwrap();
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        Item::Function(function) => {
            assert_eq!(function.name, "principal");
            assert!(function.params.is_empty());
            assert_eq!(function.body.stmts.len(), 1);
        }
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_de_if_else() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez verdade {
                mimo 1;
            } senao {
                mimo 0;
            }
        }";
    let program = parse(code).unwrap();
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::If(if_stmt) => {
                assert!(if_stmt.else_branch.is_some());
                assert_eq!(if_stmt.span.start.line, 4);
                assert_eq!(if_stmt.span.end.line, 8);
            }
            _ => panic!("stmt esperado: if"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_de_atribuicao() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 1;
            x = 2;
            mimo x;
        }";
    let program = parse(code).unwrap();
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[1] {
            Stmt::Assign(assign) => {
                assert_eq!(assign.name, "x");
                assert_eq!(assign.span.start.line, 5);
            }
            _ => panic!("stmt esperado: assign"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_preserva_span_de_chamada() {
    let code = "
        pacote main;
        carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
        carinho principal() -> bombom { mimo soma(1, 2); }";
    let program = parse(code).unwrap();
    match &program.items[1] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::Return(ret) => match &ret.expr {
                Some(expr) => match &expr.kind {
                    ExprKind::Call(_, args) => {
                        assert_eq!(args.len(), 2);
                        assert_eq!(expr.span.start.line, 4);
                        assert!(expr.span.start.col < expr.span.end.col);
                    }
                    _ => panic!("expr esperada: call"),
                },
                None => panic!("return deveria ter expressão"),
            },
            _ => panic!("stmt esperado: return"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn erro_sintatico_expected_vs_found_e_span() {
    let err = parse_and_check("pacote main; carinho principal() -> bombom { nova x = ; mimo 0; }")
        .unwrap_err()
        .to_string();
    assert_eq!(err, "Erro Sintático: expressão inválida: ';' em 1:55..1:56");
}

#[test]
fn parser_de_sempre_que() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            sempre que verdade {
                mimo 1;
            }
            mimo 0;
        }";
    let program = parse(code).unwrap();
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::While(while_stmt) => {
                assert_eq!(while_stmt.span.start.line, 4);
                assert_eq!(while_stmt.span.end.line, 6);
            }
            _ => panic!("stmt esperado: while"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_aceita_quebrar_dentro_de_sempre_que() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            sempre que verdade {
                quebrar;
            }
            mimo 0;
        }";
    let program = parse(code).unwrap();
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::While(while_stmt) => match &while_stmt.body.stmts[0] {
                Stmt::Break(_) => {}
                _ => panic!("stmt esperado: break"),
            },
            _ => panic!("stmt esperado: while"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_aceita_continuar_dentro_de_sempre_que() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 {
                x = x + 1;
                continuar;
            }
            mimo x;
        }
    "#;

    let program = parse(source).expect("parser deve aceitar continuar");
    let func = match &program.items[0] {
        Item::Function(f) => f,
        _ => panic!("item esperado: function"),
    };
    let while_stmt = match &func.body.stmts[1] {
        Stmt::While(w) => w,
        _ => panic!("stmt esperado: while"),
    };
    match &while_stmt.body.stmts[1] {
        Stmt::Continue(_) => {}
        _ => panic!("stmt esperado: continue"),
    }
}

#[test]
fn parser_aceita_expressao_com_bitwise_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo (8 & 3) | (2 ^ (1 << 4 >> 2));
        }";
    let program = parse(code).expect("parser deve aceitar bitwise básico");
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::Return(ret) => match &ret.expr {
                Some(expr) => match &expr.kind {
                    ExprKind::Binary(_, _, _) => {}
                    _ => panic!("expr esperada: binary"),
                },
                None => panic!("return deveria ter expressão"),
            },
            _ => panic!("stmt esperado: return"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_aceita_expressao_com_logicos() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez verdadeiro() || falso && verdadeiro() {
                mimo 1;
            } senao {
                mimo 0;
            }
        }
        carinho verdadeiro() -> logica { mimo verdade; }";
    let program = parse(code).expect("parser deve aceitar && e ||");
    match &program.items[0] {
        Item::Function(function) => match &function.body.stmts[0] {
            Stmt::If(if_stmt) => match &if_stmt.condition.kind {
                ExprKind::Binary(_, _, _) => {}
                _ => panic!("condição esperada: binary"),
            },
            _ => panic!("stmt esperado: if"),
        },
        _ => panic!("item esperado: função"),
    }
}

#[test]
fn parser_aceita_expressao_com_modulo_e_precedencia_multiplicativa() {
    let source = "pacote main; carinho principal() -> bombom { mimo 10 % 4 * 2 / 3; }";
    let program = parse(source).expect("parser deve aceitar %");
    let func = match &program.items[0] {
        Item::Function(f) => f,
        _ => panic!("item esperado: function"),
    };
    let ret_expr = match &func.body.stmts[0] {
        Stmt::Return(ret) => ret.expr.as_ref().expect("return com expressão"),
        _ => panic!("stmt esperado: return"),
    };
    match &ret_expr.kind {
        ExprKind::Binary(lhs_div, op_div, rhs_div) => {
            assert_eq!(op_div.name(), "Div");
            assert!(matches!(rhs_div.kind, ExprKind::IntLit(3)));
            match &lhs_div.kind {
                ExprKind::Binary(lhs_mul, op_mul, rhs_mul) => {
                    assert_eq!(op_mul.name(), "Mul");
                    assert!(matches!(rhs_mul.kind, ExprKind::IntLit(2)));
                    match &lhs_mul.kind {
                        ExprKind::Binary(_, op_mod, _) => assert_eq!(op_mod.name(), "Mod"),
                        _ => panic!("esperado nó binário com Mod"),
                    }
                }
                _ => panic!("esperado nó binário com Mul"),
            }
        }
        _ => panic!("expressão esperada: binária"),
    }
}

#[test]
fn parser_aceita_tipos_unsigned_em_assinaturas_e_locais() {
    let source = r#"
        pacote main;
        carinho soma_u8(a: u8, b: u8) -> u8 { mimo a + b; }
        carinho soma_u16(a: u16, b: u16) -> u16 { mimo a + b; }
        carinho soma_u32(a: u32, b: u32) -> u32 { mimo a + b; }
        carinho soma_u64(a: u64, b: u64) -> u64 { mimo a + b; }
        carinho principal() -> bombom {
            nova x: u8 = soma_u8(1, 2);
            nova y: u16 = soma_u16(3, 4);
            nova z: u32 = soma_u32(5, 6);
            nova w: u64 = soma_u64(40, 2);
            mimo w;
        }
    "#;
    let program = parse(source).expect("parser deve aceitar unsigned fixos");
    assert_eq!(program.items.len(), 5);
}

#[test]
fn parser_aceita_tipos_signed_em_assinaturas_e_locais_com_negacao() {
    let source = r#"
        pacote main;
        carinho soma_i8(a: i8, b: i8) -> i8 { mimo a + b; }
        carinho soma_i16(a: i16, b: i16) -> i16 { mimo a + b; }
        carinho soma_i32(a: i32, b: i32) -> i32 { mimo a + b; }
        carinho soma_i64(a: i64, b: i64) -> i64 { mimo a + b; }
        carinho principal() -> bombom {
            nova x: i8 = soma_i8(-1, 2);
            nova y: i16 = soma_i16(-3, 4);
            nova z: i32 = soma_i32(-5, 6);
            nova w: i64 = soma_i64(-40, 2);
            mimo 42;
        }
    "#;
    let program = parse(source).expect("parser deve aceitar signed fixos");
    assert_eq!(program.items.len(), 5);
}
