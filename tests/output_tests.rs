mod common;

use common::{render_ast, render_json_ast};

#[test]
fn json_ast_basico_e_estavel() {
    let json = render_json_ast("pacote main; carinho principal() -> bombom { mimo 0; }").unwrap();
    let expected = r#"{
  "node": "Program",
  "span": {
    "start": {
      "line": 1,
      "col": 1
    },
    "end": {
      "line": 1,
      "col": 55
    }
  },
  "package": {
    "node": "PackageDecl",
    "name": "main",
    "span": {
      "start": {
        "line": 1,
        "col": 1
      },
      "end": {
        "line": 1,
        "col": 13
      }
    }
  },
  "items": [
    {
      "node": "FunctionDecl",
      "name": "principal",
      "span": {
        "start": {
          "line": 1,
          "col": 14
        },
        "end": {
          "line": 1,
          "col": 55
        }
      },
      "params": [],
      "ret_type": {
        "node": "Type",
        "name": "bombom",
        "span": {
          "start": {
            "line": 1,
            "col": 37
          },
          "end": {
            "line": 1,
            "col": 43
          }
        }
      },
      "body": {
        "node": "Block",
        "span": {
          "start": {
            "line": 1,
            "col": 44
          },
          "end": {
            "line": 1,
            "col": 55
          }
        },
        "stmts": [
          {
            "node": "ReturnStmt",
            "span": {
              "start": {
                "line": 1,
                "col": 46
              },
              "end": {
                "line": 1,
                "col": 53
              }
            },
            "expr": {
              "node": "IntLit",
              "span": {
                "start": {
                  "line": 1,
                  "col": 51
                },
                "end": {
                  "line": 1,
                  "col": 52
                }
              },
              "value": 0
            }
          }
        ]
      }
    }
  ]
}"#;
    assert_eq!(json, expected);
}

#[test]
fn json_ast_multiplas_funcoes_em_items_e_valido() {
    let json = render_json_ast(
        "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }",
    )
    .unwrap();
    let expected = r#"{
  "node": "Program",
  "span": {
    "start": {
      "line": 1,
      "col": 1
    },
    "end": {
      "line": 3,
      "col": 51
    }
  },
  "package": {
    "node": "PackageDecl",
    "name": "main",
    "span": {
      "start": {
        "line": 1,
        "col": 1
      },
      "end": {
        "line": 1,
        "col": 13
      }
    }
  },
  "items": [
    {
      "node": "FunctionDecl",
      "name": "soma",
      "span": {
        "start": {
          "line": 2,
          "col": 1
        },
        "end": {
          "line": 2,
          "col": 61
        }
      },
      "params": [
        {
          "node": "Param",
          "name": "x",
          "span": {
            "start": {
              "line": 2,
              "col": 14
            },
            "end": {
              "line": 2,
              "col": 23
            }
          },
          "ty": {
            "node": "Type",
            "name": "bombom",
            "span": {
              "start": {
                "line": 2,
                "col": 17
              },
              "end": {
                "line": 2,
                "col": 23
              }
            }
          }
        },
        {
          "node": "Param",
          "name": "y",
          "span": {
            "start": {
              "line": 2,
              "col": 25
            },
            "end": {
              "line": 2,
              "col": 34
            }
          },
          "ty": {
            "node": "Type",
            "name": "bombom",
            "span": {
              "start": {
                "line": 2,
                "col": 28
              },
              "end": {
                "line": 2,
                "col": 34
              }
            }
          }
        }
      ],
      "ret_type": {
        "node": "Type",
        "name": "bombom",
        "span": {
          "start": {
            "line": 2,
            "col": 39
          },
          "end": {
            "line": 2,
            "col": 45
          }
        }
      },
      "body": {
        "node": "Block",
        "span": {
          "start": {
            "line": 2,
            "col": 46
          },
          "end": {
            "line": 2,
            "col": 61
          }
        },
        "stmts": [
          {
            "node": "ReturnStmt",
            "span": {
              "start": {
                "line": 2,
                "col": 48
              },
              "end": {
                "line": 2,
                "col": 59
              }
            },
            "expr": {
              "node": "BinaryExpr",
              "span": {
                "start": {
                  "line": 2,
                  "col": 53
                },
                "end": {
                  "line": 2,
                  "col": 58
                }
              },
              "op": "Add",
              "lhs": {
                "node": "IdentExpr",
                "span": {
                  "start": {
                    "line": 2,
                    "col": 53
                  },
                  "end": {
                    "line": 2,
                    "col": 54
                  }
                },
                "name": "x"
              },
              "rhs": {
                "node": "IdentExpr",
                "span": {
                  "start": {
                    "line": 2,
                    "col": 57
                  },
                  "end": {
                    "line": 2,
                    "col": 58
                  }
                },
                "name": "y"
              }
            }
          }
        ]
      }
    },
    {
      "node": "FunctionDecl",
      "name": "principal",
      "span": {
        "start": {
          "line": 3,
          "col": 1
        },
        "end": {
          "line": 3,
          "col": 51
        }
      },
      "params": [],
      "ret_type": {
        "node": "Type",
        "name": "bombom",
        "span": {
          "start": {
            "line": 3,
            "col": 24
          },
          "end": {
            "line": 3,
            "col": 30
          }
        }
      },
      "body": {
        "node": "Block",
        "span": {
          "start": {
            "line": 3,
            "col": 31
          },
          "end": {
            "line": 3,
            "col": 51
          }
        },
        "stmts": [
          {
            "node": "ReturnStmt",
            "span": {
              "start": {
                "line": 3,
                "col": 33
              },
              "end": {
                "line": 3,
                "col": 49
              }
            },
            "expr": {
              "node": "CallExpr",
              "span": {
                "start": {
                  "line": 3,
                  "col": 38
                },
                "end": {
                  "line": 3,
                  "col": 48
                }
              },
              "callee": {
                "node": "IdentExpr",
                "span": {
                  "start": {
                    "line": 3,
                    "col": 38
                  },
                  "end": {
                    "line": 3,
                    "col": 42
                  }
                },
                "name": "soma"
              },
              "args": [
                {
                  "node": "IntLit",
                  "span": {
                    "start": {
                      "line": 3,
                      "col": 43
                    },
                    "end": {
                      "line": 3,
                      "col": 44
                    }
                  },
                  "value": 1
                },
                {
                  "node": "IntLit",
                  "span": {
                    "start": {
                      "line": 3,
                      "col": 46
                    },
                    "end": {
                      "line": 3,
                      "col": 47
                    }
                  },
                  "value": 2
                }
              ]
            }
          }
        ]
      }
    }
  ]
}"#;
    assert_eq!(json, expected);
}

#[test]
fn json_ast_multiplos_statements_em_bloco_e_valido() {
    let json = render_json_ast(
        "\
pacote main;
carinho principal() -> bombom { nova mut x = 1; x = 2; mimo x; }",
    )
    .unwrap();
    assert!(json.contains(
        "\"stmts\": [\n          {\n            \"node\": \"LetStmt\""
    ));
    assert!(json.contains(
        "          },\n          {\n            \"node\": \"AssignStmt\""
    ));
    assert!(json.contains(
        "          },\n          {\n            \"node\": \"ReturnStmt\""
    ));
}

#[test]
fn json_ast_retorno_ausente_usa_null_de_forma_consistente() {
    let json = render_json_ast(
        "\
pacote main;
carinho log() { mimo; }
carinho principal() -> bombom { log(); mimo 0; }",
    )
    .unwrap();
    assert!(json.contains("\"ret_type\": null"));
    assert!(json.contains("\"expr\": null"));
}

#[test]
fn ast_textual_basica_e_estavel() {
    let ast = render_ast("pacote main; carinho principal() -> bombom { mimo 0; }").unwrap();
    let expected = "\
Program [1:1..1:55]
  Package main [1:1..1:13]
  Function principal -> bombom [1:14..1:55]
    Params []
    Body [1:44..1:55]
      Return [1:46..1:53]
        value IntLit(0) [1:51..1:52]
";
    assert_eq!(ast, expected);
}

#[test]
fn ast_textual_if_else_tem_formato_estavel() {
    let code = "\
pacote main;

carinho principal() -> bombom {
    talvez verdade {
        mimo 1;
    } senao {
        mimo 0;
    }
}";
    let ast = render_ast(code).unwrap();
    let expected = "\
Program [1:1..9:2]
  Package main [1:1..1:13]
  Function principal -> bombom [3:1..9:2]
    Params []
    Body [3:31..9:2]
      If [4:5..8:6]
        condition BoolLit(true) [4:12..4:19]
        then [4:20..6:6]
          Return [5:9..5:16]
            value IntLit(1) [5:14..5:15]
        else [6:13..8:6]
          Return [7:9..7:16]
            value IntLit(0) [7:14..7:15]
";
    assert_eq!(ast, expected);
}
