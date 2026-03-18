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
