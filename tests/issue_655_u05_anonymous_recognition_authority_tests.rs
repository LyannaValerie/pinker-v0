//! U-05 / TC-05 — reconhecer um callable anônimo é UMA decisão.
//!
//! A cunhagem já tinha autoridade única em [`pinker_v0::anonymous_identity`].
//! O que estava espalhado era a pergunta inversa:
//!
//! ```text
//! IS_THIS_NAME_A_COMPILER_MATERIALIZED_ANONYMOUS_CALLABLE?
//! ```
//!
//! Ela era rederivada por prefixo em AST, parser, semantic, IR, resolução de
//! módulos e CLI. Agora toda fase pergunta a
//! [`pinker_v0::anonymous_identity::is_anonymous_callable_name`], que lê o mesmo
//! prefixo que a cunhagem escreve.
//!
//! # O que esta suíte prova, e o que não prova
//!
//! A garantia terminal é por EXECUÇÃO e mora em
//! `src/anonymous_identity/recognition_oracle.rs`: a grafia do namespace é
//! renomeada por um contrafactual `#[cfg(test)]` e cada consumidor real tem de
//! acompanhar. Esta suíte guarda o que aquele oráculo não alcança.
//!
//! `T1` fixa o observável do consumidor cuja decisão só aparece na forma da IR —
//! a especialização estática de um `carinho` anônimo passado direto como
//! argumento. O caminho não especializado devolve o MESMO valor, então sem esta
//! testemunha uma regra local divergente ali seguia verde na suíte inteira.
//!
//! `T2` é censo textual, e é SUPLEMENTAR por desenho: `concat!`, `strip_prefix`
//! ou um helper intermediário reintroduzem a decisão sem repetir o literal. Ele
//! pega a regressão barata, não é a prova — e é exatamente por isso que o
//! oráculo contrafactual existe.
//!
//! `T3` fixa por que o disjunto anônimo da recusa de endereço cru é defensivo:
//! a grafia reservada não sobrevive à fronteira léxica, então nenhum
//! identificador de fonte chega àquela decisão com um nome de closure.

mod common;

use common::render_ir;
use pinker_v0::anonymous_identity::ANONYMOUS_CALLABLE_PREFIX;
use std::fs;
use std::path::Path;

/// Um `carinho` anônimo passado DIRETO como argumento de chamada. O parser
/// reconhece o nome sintético no argumento e especializa a chamada
/// estaticamente (`__fnparam_<função>_p<índice>_<closure>`); um consumidor que
/// deixasse de reconhecê-lo passaria a closure como valor de runtime, com o
/// mesmo resultado observável e outra IR.
const FONTE_ARGUMENTO_DIRETO: &str = "\
pacote main;

carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
    mimo f(x);
}

carinho principal() -> bombom {
    nova a: bombom = aplicar(carinho(v: bombom) -> bombom {
        mimo v * 2;
    }, 21);
    mimo a;
}
";

/// O nome sintético é lido da própria IR pela autoridade de cunhagem, nunca
/// transcrito: a suíte continua válida se a grafia do namespace mudar.
fn nome_anonimo_unico(ir: &str) -> String {
    let mut nomes: Vec<String> = ir
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|palavra| palavra.starts_with(ANONYMOUS_CALLABLE_PREFIX))
        .map(str::to_string)
        .collect();
    nomes.sort();
    nomes.dedup();
    assert_eq!(nomes.len(), 1, "esperava uma closure sintética: {nomes:?}");
    nomes.remove(0)
}

/// T1 — a especialização estática do argumento depende de reconhecer o nome
/// sintético, e é a única consequência observável dessa decisão.
#[test]
fn argumento_carinho_anonimo_direto_e_especializado_estaticamente() {
    let ir = render_ir(FONTE_ARGUMENTO_DIRETO).expect("IR do argumento direto");
    let anonimo = nome_anonimo_unico(&ir);
    let especializada = format!("__fnparam_aplicar_p0_{anonimo}");
    assert!(
        ir.contains(&format!("func {especializada}")),
        "o argumento anônimo tem de virar especialização estática; IR:\n{ir}"
    );
    assert!(
        ir.contains(&format!("call {especializada}(")),
        "a chamada tem de ir à especialização, não a um handle de runtime; IR:\n{ir}"
    );
}

/// Percorre `src/**/*.rs` fora da autoridade e devolve as linhas de código
/// (comentário não conta) que ainda conhecem a forma do namespace por conta
/// própria.
fn rederivacoes_em_producao() -> Vec<String> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let autoridade = raiz.join("anonymous_identity.rs");
    let mut achados = Vec::new();
    let mut pilha = vec![raiz];
    while let Some(dir) = pilha.pop() {
        for entrada in fs::read_dir(&dir).expect("ler diretório de src") {
            let caminho = entrada.expect("entrada de diretório").path();
            if caminho.is_dir() {
                pilha.push(caminho);
                continue;
            }
            if caminho.extension().map_or(true, |ext| ext != "rs") || caminho == autoridade {
                continue;
            }
            let fonte = fs::read_to_string(&caminho).expect("ler fonte");
            for (numero, linha) in fonte.lines().enumerate() {
                let cru = linha.trim_start();
                if cru.starts_with("//") {
                    continue;
                }
                let decide_por_conta_propria = linha.contains(ANONYMOUS_CALLABLE_PREFIX)
                    || ((linha.contains("starts_with") || linha.contains("strip_prefix"))
                        && linha.contains("ANONYMOUS_CALLABLE_PREFIX"));
                if decide_por_conta_propria {
                    achados.push(format!("{}:{}: {}", caminho.display(), numero + 1, cru));
                }
            }
        }
    }
    achados
}

/// T2 — guarda textual suplementar: nenhum arquivo de produção fora da
/// autoridade escreve a forma do namespace nem aplica a regra de prefixo.
#[test]
fn nenhuma_rederivacao_textual_sobrevive_em_producao() {
    let achados = rederivacoes_em_producao();
    assert!(
        achados.is_empty(),
        "reconhecimento de callable anônimo rederivado fora da autoridade:\n{}",
        achados.join("\n")
    );
}

/// T3 — a grafia reservada não atravessa a fronteira léxica, então nenhum
/// identificador escrito na fonte chega às fases como nome de closure.
#[test]
fn grafia_reservada_nao_atravessa_a_fronteira_lexica() {
    // A posição de declaração e a posição `&IDENT` — a única que alimenta a
    // recusa de endereço cru — são recusadas pela mesma fronteira.
    let posicoes = [
        format!("    nova {ANONYMOUS_CALLABLE_PREFIX}x: bombom = 1;"),
        format!("    nova p: seta<bombom> = &{ANONYMOUS_CALLABLE_PREFIX}x;"),
    ];
    for posicao in posicoes {
        let fonte = format!(
            "pacote main;\n\ncarinho principal() -> bombom {{\n{posicao}\n    mimo 0;\n}}\n"
        );
        let erro = render_ir(&fonte).expect_err("identificador reservado tem de ser recusado");
        let texto = format!("{erro:?}");
        assert!(
            texto.contains("reservado"),
            "a recusa tem de ser a do namespace reservado em `{posicao}`: {texto}"
        );
    }
}
