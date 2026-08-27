//! HR3 — fechamento das evidências terminais de payload de união.
//!
//! Três propriedades que a cobertura anterior deixou apenas **classificadas**
//! passam a ser **executadas**:
//!
//! 1. agregado aninhado — um `ninho` cujo storage contém um array fixo;
//! 2. agregados nominais homorrepresentados — `ninho`s distintos com o mesmo
//!    `TypeIR`, tamanho e alinhamento, separados só pelo `ResolvedTypeId`;
//! 3. mutação do binding extraído — o storage do braço é do braço, e a
//!    extração seguinte continua devolvendo o snapshot original.
//!
//! As três são exercitadas por programas Pinker válidos, no interpretador e no
//! binário ELF nativo. A única forma que a sintaxe-fonte ainda não constrói
//! diretamente é a **leitura célula a célula do agregado interno**: não há
//! construtor de array literal nem acesso encadeado a campo agregado. Essa
//! parte é provada por um programa IR sintético — a IR vem do lowering real,
//! recebe uma cirurgia mínima e atravessa integralmente o pipeline real
//! (`ir_validate`, `cfg_ir`, `cfg_ir_validate`, `instr_select`,
//! `instr_select_validate`, `abstract_machine`, `abstract_machine_validate`)
//! antes de executar no interpretador e no nativo.
//!
//! Nenhum teste deste arquivo prova execução por classificação: toda asserção
//! observa bytes efetivamente copiados.

mod common;

use common::ControlledCommand as Command;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pinker_v0::ast::{StructDecl, Type};
use pinker_v0::interpreter::RuntimeValue;
use pinker_v0::ir::{
    BinaryOpIR, BlockIR, FalarArgIR, FunctionIR, InstructionIR, ProgramIR, TypeIR, UnionMatchIR,
    ValueIR,
};
use pinker_v0::token::{Position, Span};
use pinker_v0::union_payload::{classify_union_payload, UnionPayloadRepresentation};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, backend_s, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, interpreter, ir, ir_validate, semantic,
};

const POSICAO: Position = Position { line: 1, col: 1 };
const SPAN: Span = Span {
    start: POSICAO,
    end: POSICAO,
    source: pinker_v0::source_map::SourceId::UNKNOWN,
};

/// Sequência para diretórios temporários únicos por binário nativo montado.
static SEQUENCIA_NATIVA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Execução de exemplos versionados
// ---------------------------------------------------------------------------

fn exemplo(nome: &str) -> String {
    format!("examples/{nome}.pink")
}

/// Executa um exemplo no interpretador, pelo binário `pink`.
fn interpretado(caminho: &str) -> Vec<String> {
    let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", caminho])
        .output()
        .expect("execução do interpretador");
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(
        saida.stderr.is_empty(),
        "stderr do interpretador deveria ser vazio: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    linhas(&saida.stdout)
}

fn linhas(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_string)
        .collect()
}

fn diretorio_temporario(prefixo: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let sequencia = SEQUENCIA_NATIVA.fetch_add(1, Ordering::SeqCst);
    let caminho = std::env::temp_dir().join(format!("{prefixo}_{nanos}_{sequencia}"));
    std::fs::create_dir_all(&caminho).expect("diretório temporário");
    caminho
}

/// Compila um exemplo para ELF nativo, executa e devolve as linhas de `falar`.
fn nativo_do_exemplo(caminho: &str, teste: &str) -> Option<Vec<String>> {
    let (_driver, runtime_lib) = common::require_native_evidence(teste, true)?;
    let runtime_lib = runtime_lib.expect("runtime nativo exigido");
    let diretorio = diretorio_temporario("pinker_hr3_exemplo");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&diretorio)
        .arg(caminho)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("invocação de pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let stem = std::path::Path::new(caminho)
        .file_stem()
        .expect("nome do exemplo")
        .to_string_lossy()
        .to_string();
    let execucao = Command::new(diretorio.join(stem))
        .output()
        .expect("execução do binário nativo");
    assert!(
        execucao.status.success(),
        "binário nativo falhou: {}",
        String::from_utf8_lossy(&execucao.stderr)
    );
    let resultado = linhas(&execucao.stdout);
    let _ = std::fs::remove_dir_all(&diretorio);
    Some(resultado)
}

/// Executa nos dois motores e exige a mesma saída.
fn paridade(nome: &str, esperado: &[&str], teste: &str) {
    let caminho = exemplo(nome);
    assert_eq!(interpretado(&caminho), esperado, "interpretador");
    if let Some(linhas) = nativo_do_exemplo(&caminho, teste) {
        assert_eq!(linhas, esperado, "nativo");
    }
}

// ---------------------------------------------------------------------------
// GAP 1 — agregado aninhado
// ---------------------------------------------------------------------------

const NINHADO: &str = "hr3_uniao_agregado_aninhado_valido";

#[test]
fn hr3_agregado_aninhado_contem_outro_agregado() {
    let fonte = std::fs::read_to_string(exemplo(NINHADO)).expect("exemplo");
    let programa = ir_valida(&fonte);
    let uniao = programa.union_types.first().expect("união registrada");
    let membro = uniao
        .members
        .iter()
        .find(|membro| membro.ty == TypeIR::Struct)
        .expect("membro agregado");
    assert_eq!(
        membro.payload_layout.representation,
        UnionPayloadRepresentation::Aggregate
    );
    assert_eq!(
        membro.payload_layout.size, 40,
        "cabeça + array de três + cauda"
    );
    assert_eq!(membro.payload_layout.align, 8);

    // A camada interna é ela mesma um agregado: o storage do payload contém
    // outro agregado, e não apenas escalares.
    let (aliases, structs): (HashMap<String, Type>, HashMap<String, StructDecl>) =
        (HashMap::new(), HashMap::new());
    let interno = classify_union_payload(
        &Type::FixedArray {
            element: Box::new(Type::Bombom(SPAN)),
            size: 3,
            span: SPAN,
        },
        &aliases,
        &structs,
    )
    .expect("classificação do array interno");
    assert_eq!(
        interno.representation,
        UnionPayloadRepresentation::Aggregate,
        "o storage interno também é agregado"
    );
    assert_eq!(interno.size, 24);
}

#[test]
fn hr3_agregado_aninhado_executa_no_interpretador_e_no_nativo() {
    // 11 e 15 são cabeça e cauda do snapshot, separadas pelo array interno de
    // 24 bytes; 91 é a origem já modificada depois da injeção.
    paridade(
        NINHADO,
        &["11", "15", "91"],
        concat!(module_path!(), ":", line!()),
    );
}

// ---------------------------------------------------------------------------
// GAP 2 — agregados nominais homorrepresentados
// ---------------------------------------------------------------------------

const NOMINAIS: &str = "hr3_uniao_agregados_nominais_valido";

#[test]
fn hr3_agregados_nominais_compartilham_representacao_e_divergem_em_identidade() {
    let fonte = std::fs::read_to_string(exemplo(NOMINAIS)).expect("exemplo");
    let programa = ir_valida(&fonte);
    let uniao = programa.union_types.first().expect("união registrada");
    assert_eq!(uniao.members.len(), 2);
    let alfa = &uniao.members[0];
    let beta = &uniao.members[1];

    assert_eq!(alfa.ty, TypeIR::Struct);
    assert_eq!(beta.ty, TypeIR::Struct);
    assert_eq!(
        alfa.payload_layout, beta.payload_layout,
        "tamanho, alinhamento e categoria idênticos"
    );
    assert_eq!(alfa.payload_layout.size, 16);
    assert_eq!(alfa.payload_layout.align, 8);
    assert_ne!(
        alfa.resolved_type_id, beta.resolved_type_id,
        "a identidade nominal separa os membros homorrepresentados"
    );
    assert_ne!(alfa.canonical_member_key, beta.canonical_member_key);
    assert_ne!(alfa.tag, beta.tag);

    let nominais: Vec<Option<&str>> = [alfa, beta]
        .iter()
        .map(|membro| {
            programa
                .resolved_types
                .iter()
                .find(|entrada| entrada.id == membro.resolved_type_id)
                .and_then(|entrada| entrada.nominal_name.as_deref())
        })
        .collect();
    assert_eq!(nominais, vec![Some("Alfa"), Some("Beta")]);
}

#[test]
fn hr3_agregado_nominal_alfa_e_beta_executam_com_paridade() {
    // Alfa → 41/42; Beta → 51/52; reinjeção de Alfa → 41/42 de novo.
    paridade(
        NOMINAIS,
        &["41", "42", "51", "52", "41", "42"],
        concat!(module_path!(), ":", line!()),
    );
}

// ---------------------------------------------------------------------------
// GAP 3 — mutação do binding extraído
// ---------------------------------------------------------------------------

const MUTACAO: &str = "hr3_uniao_binding_extraido_mutavel_valido";

#[test]
fn hr3_mutacao_do_binding_extraido_nao_altera_o_snapshot() {
    // 11/12 é a segunda extração, feita depois de o primeiro binding já ter
    // sido modificado; 77/78 é o primeiro binding lido **de novo**, ainda com a
    // mutação — os dois storages coexistem; 11 final é a origem intacta.
    paridade(
        MUTACAO,
        &["11", "12", "77", "78", "11"],
        concat!(module_path!(), ":", line!()),
    );
}

#[test]
fn hr3_backend_separa_o_storage_das_duas_extracoes() {
    let fonte = std::fs::read_to_string(exemplo(MUTACAO)).expect("exemplo");
    let (selected, _machine) = pipeline(&ir_valida(&fonte));
    let assembly = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    // O destino de cada extração é carregado em `%r9` imediatamente antes da
    // chamada; duas extrações nunca podem compartilhar o mesmo endereço.
    let linhas: Vec<&str> = assembly.lines().map(str::trim).collect();
    let destinos: Vec<&str> = linhas
        .windows(2)
        .filter(|par| par[1].contains("call pinker_uniao_copiar_payload"))
        .map(|par| par[0])
        .collect();
    assert!(
        destinos.len() >= 2,
        "as duas extrações do exemplo emitem cópias validadas: {destinos:?}"
    );
    for destino in &destinos {
        assert!(
            destino.starts_with("leaq") && destino.ends_with("%r9"),
            "o destino da extração é um storage do frame do chamador: {destino}"
        );
    }
    let distintos: std::collections::BTreeSet<&&str> = destinos.iter().collect();
    assert_eq!(
        distintos.len(),
        destinos.len(),
        "cada extração materializa um storage próprio: {destinos:?}"
    );
}

// ---------------------------------------------------------------------------
// Pipeline real sobre IR sintética
// ---------------------------------------------------------------------------

/// Abaixa um programa Pinker até a IR validada.
fn ir_valida(source: &str) -> ProgramIR {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantica");
    let programa = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&programa).expect("ir validate");
    programa
}

/// Conduz uma IR — real ou sintética — pelo restante do pipeline real, com
/// todos os validadores aplicados em cada fronteira.
fn pipeline(
    programa: &ProgramIR,
) -> (
    instr_select::SelectedProgram,
    abstract_machine::MachineProgram,
) {
    ir_validate::validate_program(programa).expect("ir validate");
    let cfg = cfg_ir::lower_program(programa).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    (selected, machine)
}

fn principal_mut(programa: &mut ProgramIR) -> &mut FunctionIR {
    programa
        .functions
        .iter_mut()
        .find(|funcao| funcao.name == "principal")
        .expect("função principal")
}

fn union_match_mut(funcao: &mut FunctionIR) -> &mut UnionMatchIR {
    funcao
        .entry
        .instructions
        .iter_mut()
        .find_map(|instrucao| match instrucao {
            InstructionIR::UnionMatch(encaixe) => Some(encaixe),
            _ => None,
        })
        .expect("instrução de encaixe de união")
}

/// Lê a palavra do agregado no deslocamento `offset`, em bytes.
///
/// `FieldAccess` é a fronteira tipada de acesso a campo da IR: a base é o valor
/// agregado — que já é o endereço do seu storage — e o deslocamento é explícito.
/// A fonte não oferece nome de campo para as células do array interno; a IR
/// oferece o deslocamento.
fn palavra(slot: &str, offset: u64) -> ValueIR {
    ValueIR::FieldAccess {
        base: Box::new(ValueIR::Local(slot.to_string())),
        field: format!("__hr3_offset_{offset}"),
        field_offset: offset,
        result_type: TypeIR::Bombom,
    }
}

fn falar(valor: ValueIR) -> InstructionIR {
    InstructionIR::Falar {
        args: vec![FalarArgIR {
            value: valor,
            ty: TypeIR::Bombom,
        }],
        span: SPAN,
    }
}

/// Combina as palavras observadas num único `bombom`, com peso por posição: o
/// valor devolvido identifica conteúdo **e** offset.
fn combina(palavras: &[(ValueIR, u64)]) -> ValueIR {
    let mut acumulado: Option<ValueIR> = None;
    for (valor, peso) in palavras {
        let termo = ValueIR::Binary {
            op: BinaryOpIR::Mul,
            lhs: Box::new(valor.clone()),
            rhs: Box::new(ValueIR::Int(*peso)),
            ty: TypeIR::Bombom,
        };
        acumulado = Some(match acumulado {
            None => termo,
            Some(anterior) => ValueIR::Binary {
                op: BinaryOpIR::Add,
                lhs: Box::new(anterior),
                rhs: Box::new(termo),
                ty: TypeIR::Bombom,
            },
        });
    }
    acumulado.expect("ao menos uma palavra observada")
}

/// Fonte do programa sintético: idêntica ao exemplo versionado, mas com um
/// local mutável que recebe o valor combinado das cinco palavras do payload.
const FONTE_SINTETICA: &str = r#"
pacote main; trazer memoria.alocar;

ninho Ninhado {
    cabeca: bombom;
    miolo: [bombom; 3];
    cauda: bombom;
}

carinho principal() -> bombom {
    nova muda resultado: bombom = 0;
    nova base: seta<Ninhado> = alocar(40) virar seta<Ninhado>;
    (*base).cabeca = 11;
    (*base).cauda = 15;

    nova valor: uniao<Ninhado, u8> = (*base) virar uniao<Ninhado, u8>;

    (*base).cabeca = 91;
    (*base).cauda = 95;

    encaixe valor {
        caso Ninhado(ninhado) {
            resultado = 1;
        }
        caso u8(numero) {
            resultado = 2;
        }
    }

    mimo resultado;
}
"#;

/// Pesos posicionais: o valor combinado identifica conteúdo e offset.
const PESOS: [u64; 5] = [1, 100, 10_000, 1_000_000, 100_000_000];

/// Constrói o programa IR sintético que lê as cinco palavras do payload.
///
/// A cirurgia é mínima e explícita: escreve as células do array interno pela
/// fronteira tipada de campo com deslocamento e substitui o corpo do braço por
/// leituras integrais do binding.
fn programa_sintetico() -> (ProgramIR, [u64; 5]) {
    let mut programa = ir_valida(FONTE_SINTETICA);
    let funcao = principal_mut(&mut programa);
    let slot_resultado = funcao
        .locals
        .iter()
        .find(|local| local.source_name == "resultado")
        .expect("local resultado")
        .slot
        .clone();
    let slot_base = funcao
        .locals
        .iter()
        .find(|local| local.source_name == "base")
        .expect("local base")
        .slot
        .clone();

    // As três células do array interno não têm nome de campo na fonte.
    let indice_injecao = funcao
        .entry
        .instructions
        .iter()
        .position(|instrucao| {
            matches!(
                instrucao,
                InstructionIR::Let {
                    value: ValueIR::UnionInject { .. },
                    ..
                }
            )
        })
        .expect("injeção de união");
    let celulas: [u64; 3] = [12, 13, 14];
    for (posicao, valor) in celulas.iter().enumerate() {
        funcao.entry.instructions.insert(
            indice_injecao + posicao,
            InstructionIR::StoreFieldIndirect {
                base: ValueIR::Deref {
                    ptr: Box::new(ValueIR::Local(slot_base.clone())),
                    result_type: TypeIR::Struct,
                    is_volatile: false,
                },
                field: format!("__hr3_miolo_{posicao}"),
                field_offset: 8 + posicao as u64 * 8,
                value: ValueIR::Int(*valor),
                value_type: TypeIR::Bombom,
                is_volatile: false,
                span: SPAN,
            },
        );
    }

    let encaixe = union_match_mut(funcao);
    let slot_ninhado = encaixe.arms[0].binding.slot.clone();
    let label = encaixe.arms[0].body.label.clone();
    let observadas: Vec<(ValueIR, u64)> = (0..5)
        .map(|posicao| (palavra(&slot_ninhado, posicao * 8), PESOS[posicao as usize]))
        .collect();
    let mut corpo: Vec<InstructionIR> = (0..5)
        .map(|posicao| falar(palavra(&slot_ninhado, posicao * 8)))
        .collect();
    corpo.push(InstructionIR::Assign {
        slot: slot_resultado,
        value: combina(&observadas),
        span: SPAN,
    });
    encaixe.arms[0].body = BlockIR {
        label,
        instructions: corpo,
        span: SPAN,
    };

    let esperado = [11, 12, 13, 14, 15];
    (programa, esperado)
}

fn combinado(palavras: [u64; 5]) -> u64 {
    palavras
        .iter()
        .zip(PESOS.iter())
        .map(|(palavra, peso)| palavra * peso)
        .sum()
}

#[test]
fn hr3_agregado_interno_e_copiado_celula_a_celula_no_interpretador() {
    let (programa, esperado) = programa_sintetico();
    let (_selected, machine) = pipeline(&programa);
    let retorno = match interpreter::run_program(&machine).expect("execução no interpretador") {
        Some(RuntimeValue::Int(valor)) => valor,
        outro => panic!("retorno inesperado: {outro:?}"),
    };
    assert_eq!(
        retorno,
        combinado(esperado),
        "as cinco palavras do snapshot, inclusive as do agregado interno"
    );
}

#[test]
fn hr3_agregado_interno_e_copiado_celula_a_celula_no_nativo() {
    let (programa, esperado) = programa_sintetico();
    let (selected, _machine) = pipeline(&programa);
    let Some((_driver, runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let runtime_lib = runtime_lib.expect("runtime nativo exigido");
    let assembly = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");

    let diretorio = diretorio_temporario("pinker_hr3_sintetico");
    let caminho_asm = diretorio.join("programa.s");
    let caminho_bin = diretorio.join("programa");
    std::fs::write(&caminho_asm, assembly).expect("gravação do assembly");
    let ligacao = Command::new("cc")
        .arg(&caminho_asm)
        .arg(&runtime_lib)
        .arg("-lpthread")
        .arg("-ldl")
        .arg("-lm")
        .arg("-o")
        .arg(&caminho_bin)
        .output()
        .expect("invocação do driver C");
    assert!(
        ligacao.status.success(),
        "montagem/ligação falhou: {}",
        String::from_utf8_lossy(&ligacao.stderr)
    );
    let execucao = Command::new(&caminho_bin)
        .output()
        .expect("execução do binário nativo");
    let observado = linhas(&execucao.stdout);
    let _ = std::fs::remove_dir_all(&diretorio);

    let esperado: Vec<String> = esperado.iter().map(|palavra| palavra.to_string()).collect();
    assert_eq!(
        observado, esperado,
        "o binário nativo copia o agregado interno célula a célula"
    );
}

// ---------------------------------------------------------------------------
// Contrato documentado
// ---------------------------------------------------------------------------

/// A documentação não pode voltar a descrever o snapshot como sendo de uma
/// palavra: o payload é copiado integralmente, com o tamanho real do membro.
#[test]
fn hr3_documentacao_nao_descreve_snapshot_de_uma_palavra() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let arquivos = [
        raiz.join("runtime/pinker_rt/src/lib.rs"),
        raiz.join("docs/union_types.md"),
        raiz.join("docs/navigation.jsonl"),
    ];
    for arquivo in arquivos {
        let conteudo = std::fs::read_to_string(&arquivo)
            .unwrap_or_else(|erro| panic!("leitura de {}: {erro}", arquivo.display()));
        for proibido in [
            "snapshot de uma palavra",
            "payload de uma palavra",
            "payload limitado a uma palavra",
            "payload_word",
        ] {
            assert!(
                !conteudo.contains(proibido),
                "{} ainda descreve o payload como '{proibido}'",
                arquivo.display()
            );
        }
    }
}
