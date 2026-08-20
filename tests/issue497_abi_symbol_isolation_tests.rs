//! Issue #497 — isolamento dos símbolos Pinker da ABI externa e injetividade da
//! renderização nativa.
//!
//! Os testes desta suíte exercitam o **caminho real do produto** (`pink build
//! --nativo`, com `libpinker_rt.a` e a ordem de link de verdade) sempre que a
//! evidência nativa está disponível, e inspecionam o artefato com `nm` e
//! `readelf` — não por casamento de string sobre o `.s`.
//!
//! Fecha F-17: a cobertura não depende do renderer hospedado, que monta com
//! `runtime_init=false` e nunca liga o archive.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::native_symbol::{
    self, NativeBinding, NativeDefinition, NativeSurface, ReservedScope,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers do caminho real do produto
// ---------------------------------------------------------------------------

/// Símbolo tal como `readelf -Ws` o descreve.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ElfSymbol {
    name: String,
    bind: String,
    symbol_type: String,
    visibility: String,
    section: String,
    size: u64,
}

impl ElfSymbol {
    fn is_defined(&self) -> bool {
        self.section != "UND"
    }
}

struct BuiltProgram {
    _artifacts: NativeArtifactDir,
    assembly: String,
    executable: PathBuf,
}

impl BuiltProgram {
    fn symbols(&self) -> Vec<ElfSymbol> {
        read_symbols(&self.executable, false)
    }

    fn dynamic_symbols(&self) -> Vec<ElfSymbol> {
        read_symbols(&self.executable, true)
    }

    fn defined(&self, name: &str) -> Option<ElfSymbol> {
        self.symbols()
            .into_iter()
            .find(|symbol| symbol.name == name && symbol.is_defined())
    }

    fn undefined(&self, name: &str) -> Vec<ElfSymbol> {
        self.symbols()
            .into_iter()
            .filter(|symbol| symbol.name.split('@').next() == Some(name) && !symbol.is_defined())
            .collect()
    }

    fn run(&self, logical_case: &str) -> (Option<i32>, String) {
        let output = Command::new(&self.executable)
            .logical_case(logical_case)
            .timeout(Duration::from_secs(10))
            .output()
            .expect("execução nativa contida");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
        )
    }
}

fn read_symbols(binary: &Path, dynamic: bool) -> Vec<ElfSymbol> {
    let flag = if dynamic { "--dyn-syms" } else { "-Ws" };
    let output = StdCommand::new("readelf")
        .arg(flag)
        .arg("-W")
        .arg(binary)
        .output()
        .expect("readelf disponível");
    assert!(output.status.success(), "readelf falhou em {binary:?}");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut symbols = Vec::new();
    for line in text.lines() {
        // Num: Value Size Type Bind Vis Ndx Name
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 || !fields[0].ends_with(':') {
            continue;
        }
        let Ok(size) = fields[2].parse::<u64>() else {
            continue;
        };
        symbols.push(ElfSymbol {
            name: fields[7].to_string(),
            symbol_type: fields[3].to_string(),
            bind: fields[4].to_string(),
            visibility: fields[5].to_string(),
            section: fields[6].to_string(),
            size,
        });
    }
    symbols
}

/// Executa `pink build --nativo` de verdade: runtime archive, ordem de link e
/// flags reais do produto. Devolve `None` quando a evidência nativa não está
/// disponível no ambiente.
fn build_nativo(test: &str, stem: &str, source: &str) -> Option<BuiltProgram> {
    let (_driver, Some(runtime_lib)) = common::require_native_evidence(test, true)? else {
        return None;
    };
    if StdCommand::new("readelf")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("{{\"event\":\"native_evidence\",\"reason\":\"readelf_not_found\",\"status\":\"unavailable\",\"test\":\"{test}\"}}");
        return None;
    }
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let source_path = artifacts.path().join(format!("{stem}.pink"));
    fs::write(&source_path, source).expect("gravar fonte temporária");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(artifacts.path())
        .arg(&source_path)
        .env("PINKER_RT_LIB", &runtime_lib)
        .logical_case("issue497-build-nativo")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo contido");
    assert!(
        build.status.success(),
        "build --nativo de '{stem}' falhou:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let assembly =
        fs::read_to_string(artifacts.path().join(format!("{stem}.s"))).expect("assembly emitido");
    Some(BuiltProgram {
        executable: artifacts.path().join(stem),
        _artifacts: artifacts,
        assembly,
    })
}

/// Saída do interpretador para a mesma fonte, pelo binário real do produto.
fn interpretar(stem: &str, source: &str) -> (Option<i32>, String) {
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let source_path = artifacts.path().join(format!("{stem}.pink"));
    fs::write(&source_path, source).expect("gravar fonte temporária");
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&source_path)
        .logical_case("issue497-interpretador")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("interpretação contida");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

fn checar(source: &str) -> Result<(), String> {
    common::parse_and_check(source).map_err(|error| error.to_string())
}

/// Programa que define um nome do host como função Pinker e o chama.
fn programa_colisao_de_funcao(nome: &str) -> String {
    format!(
        "pacote main;\n\
         \n\
         carinho {nome}(n: bombom) -> bombom {{\n\
         \x20   mimo n + 1;\n\
         }}\n\
         \n\
         carinho principal() -> bombom {{\n\
         \x20   falar({nome}(41));\n\
         \x20   mimo 0;\n\
         }}\n"
    )
}

// ---------------------------------------------------------------------------
// Seção 4 / 9 — matriz de colisão com o host, no caminho real
// ---------------------------------------------------------------------------

/// Controle: função Pinker comum e `eterno` comum permanecem corretos e ficam
/// LOCAL, com metadata ELF coerente.
#[test]
fn controle_funcao_comum_e_eterno_sao_locais_e_corretos() {
    const FONTE: &str = r#"
pacote main;

eterno BASE: bombom = 40;

carinho somar(a: bombom, b: bombom) -> bombom {
    mimo a + b;
}

carinho principal() -> bombom {
    falar(somar(BASE, 2));
    mimo 0;
}
"#;
    let Some(program) = build_nativo(
        concat!(module_path!(), ":", line!()),
        "controle_comum",
        FONTE,
    ) else {
        return;
    };

    let (codigo, saida) = program.run("issue497-controle");
    assert_eq!(codigo, Some(0));
    assert_eq!(saida, "42\n");
    assert_eq!(interpretar("controle_comum", FONTE), (Some(0), saida));

    let somar = program
        .defined("somar")
        .expect("função de usuário definida");
    assert_eq!(
        somar.bind, "LOCAL",
        "função Pinker comum não atravessa link"
    );
    assert_eq!(somar.symbol_type, "FUNC", "F-09: deixa de ser NOTYPE");
    assert!(somar.size > 0, "F-09: st_size deixa de ser 0");
    assert_eq!(
        somar.visibility, "DEFAULT",
        ".hidden não é a correção: a ligação é que muda"
    );

    let base = program.defined("BASE").expect("global eterno definida");
    assert_eq!(base.bind, "LOCAL");
    assert_eq!(base.symbol_type, "OBJECT", "dado não recebe @function");
    assert_eq!(base.size, 8);

    let main = program.defined("main").expect("entrypoint definido");
    assert_eq!(main.bind, "GLOBAL", "main é consumido pelo CRT");
    assert_eq!(main.symbol_type, "FUNC");
    assert!(main.size > 0);
    assert!(
        program.defined("principal").is_none(),
        "a grafia Pinker do entrypoint não vira símbolo própria na superfície montável"
    );
}

/// F-01: cada nome de função do host continua legal como nome Pinker, produz
/// definição LOCAL e **não** captura a referência que o runtime faz ao host.
#[test]
fn colisao_com_funcao_do_host_e_isolada_e_o_host_continua_resolvido() {
    for nome in ["malloc", "memcpy", "write", "getenv", "free"] {
        let fonte = programa_colisao_de_funcao(nome);
        checar(&fonte).unwrap_or_else(|error| {
            panic!("'{nome}' deve continuar legal como nome Pinker: {error}")
        });

        let stem = format!("colisao_{nome}");
        let Some(program) = build_nativo(concat!(module_path!(), ":", line!()), &stem, &fonte)
        else {
            return;
        };

        let (codigo, saida) = program.run("issue497-colisao-host");
        let interpretado = interpretar(&stem, &fonte);
        assert_eq!(
            (codigo, saida.as_str()),
            (Some(0), "42\n"),
            "nativo de '{nome}' divergiu"
        );
        assert_eq!(
            interpretado,
            (codigo, saida),
            "INTERPRETER_RESULT == NATIVE_RESULT para '{nome}'"
        );

        let definido = program
            .defined(nome)
            .unwrap_or_else(|| panic!("'{nome}' definido pelo programa"));
        assert_eq!(
            definido.bind, "LOCAL",
            "definição Pinker de '{nome}' não pode satisfazer referência externa"
        );
        assert_eq!(definido.symbol_type, "FUNC");

        // A propriedade central: LOCAL <nome> e UND <nome>@GLIBC coexistem.
        let undefined = program.undefined(nome);
        assert!(
            !undefined.is_empty(),
            "a dependência do host em '{nome}' deveria continuar sendo resolvida ao host"
        );
        assert!(
            undefined.iter().all(|symbol| symbol.bind == "GLOBAL"),
            "referência ao host em '{nome}' permanece global: {undefined:?}"
        );
    }
}

/// F-14: a classe de colisão não é exclusiva de funções. `eterno environ` não
/// pode substituir o `environ` do host nem aparecer na `.dynsym`.
#[test]
fn colisao_com_dado_global_do_host_e_isolada() {
    const FONTE: &str = r#"
pacote main;

eterno environ: bombom = 123;

carinho principal() -> bombom {
    falar(environ);
    mimo 0;
}
"#;
    checar(FONTE).expect("'environ' continua legal como nome Pinker");

    let Some(program) = build_nativo(
        concat!(module_path!(), ":", line!()),
        "colisao_environ",
        FONTE,
    ) else {
        return;
    };

    let (codigo, saida) = program.run("issue497-colisao-environ");
    assert_eq!((codigo, saida.as_str()), (Some(0), "123\n"));
    assert_eq!(interpretar("colisao_environ", FONTE), (Some(0), saida));

    let definido = program.defined("environ").expect("dado Pinker definido");
    assert_eq!(definido.bind, "LOCAL");
    assert_eq!(definido.symbol_type, "OBJECT");

    assert!(
        program
            .dynamic_symbols()
            .iter()
            .all(|symbol| symbol.name != "environ" || !symbol.is_defined()),
        "o executável não pode passar a DEFINIR 'environ' na .dynsym"
    );
    assert!(
        !program.undefined("environ").is_empty(),
        "a referência do host a 'environ' continua sendo resolvida ao host"
    );
}

/// Símbolos de ABI do runtime continuam externamente ligáveis: o objeto Pinker
/// os referencia como UND e o archive os resolve.
#[test]
fn simbolos_de_abi_do_runtime_continuam_externamente_ligaveis() {
    const FONTE: &str = r#"
pacote main;

carinho principal() -> bombom {
    falar(7);
    mimo 0;
}
"#;
    let Some(program) = build_nativo(concat!(module_path!(), ":", line!()), "runtime_abi", FONTE)
    else {
        return;
    };

    assert!(
        program.assembly.contains("call pinker_rt_iniciar"),
        "o prólogo do entrypoint continua chamando a inicialização do runtime"
    );
    for simbolo in ["pinker_rt_iniciar", "pinker_falar_fim"] {
        let definido = program
            .defined(simbolo)
            .unwrap_or_else(|| panic!("'{simbolo}' resolvido a partir do archive"));
        assert_eq!(
            definido.bind, "GLOBAL",
            "símbolo de ABI do runtime não pode virar local"
        );
    }
    assert!(
        !program.assembly.contains(".local pinker_rt_iniciar"),
        "o programa não define nem localiza símbolos do runtime"
    );
}

/// Nenhuma definição Pinker do programa vaza para a `.dynsym` do executável.
#[test]
fn definicoes_do_usuario_nao_vazam_para_dynsym() {
    const FONTE: &str = r#"
pacote main;

eterno SEGREDO: bombom = 5;

carinho auxiliar(n: bombom) -> bombom {
    mimo n * 2;
}

carinho principal() -> bombom {
    falar(auxiliar(SEGREDO));
    mimo 0;
}
"#;
    let Some(program) = build_nativo(
        concat!(module_path!(), ":", line!()),
        "sem_vazamento",
        FONTE,
    ) else {
        return;
    };

    let dinamicos: BTreeSet<String> = program
        .dynamic_symbols()
        .into_iter()
        .filter(ElfSymbol::is_defined)
        .map(|symbol| symbol.name)
        .collect();
    for nome in ["auxiliar", "SEGREDO"] {
        assert!(
            !dinamicos.contains(nome),
            "'{nome}' não deveria ser exportado dinamicamente: {dinamicos:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Seção 3 — autoridade explícita de entrypoint
// ---------------------------------------------------------------------------

#[test]
fn entrypoint_sozinho_produz_main_global() {
    const FONTE: &str = "pacote main;\ncarinho principal() -> bombom {\n    mimo 0;\n}\n";
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    assert!(asm.contains(".globl main"), "{asm}");
    assert!(asm.contains(".type main, @function"), "{asm}");
    assert!(
        !asm.contains(".globl principal"),
        "a grafia Pinker não vira símbolo global na superfície montável: {asm}"
    );
}

#[test]
fn carinho_main_e_recusado_antes_da_toolchain_com_span() {
    const FONTE: &str = r#"
pacote main;

carinho main() -> bombom {
    mimo 7;
}

carinho principal() -> bombom {
    mimo main();
}
"#;
    let erro = checar(FONTE).expect_err("`carinho main` deve ser recusado cedo");
    assert!(erro.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{erro}");
    assert!(erro.contains("principal"), "{erro}");
    assert!(
        erro.contains("4:1") || erro.contains("em 4:"),
        "o diagnóstico precisa de span da declaração: {erro}"
    );
    assert!(
        !erro.contains("already defined"),
        "o erro não pode vir do GNU as: {erro}"
    );
}

#[test]
fn eterno_main_tambem_e_recusado() {
    const FONTE: &str =
        "pacote main;\neterno main: bombom = 1;\ncarinho principal() -> bombom { mimo main; }\n";
    let erro = checar(FONTE).expect_err("`eterno main` deve ser recusado cedo");
    assert!(erro.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{erro}");
}

#[test]
fn pacote_main_continua_legal() {
    // `main` é nome legítimo de pacote — a reserva vale na fronteira de
    // definição produtora de símbolo, não na fronteira léxica.
    checar("pacote main;\ncarinho principal() -> bombom { mimo 0; }\n")
        .expect("`pacote main` não pode ser afetado pela reserva");
    assert!(native_symbol::reserved_namespace("main", ReservedScope::AnyIdentifier).is_none());
}

#[test]
fn entrypoint_tem_uma_autoridade_so_no_backend() {
    // Mata a mutação "o mapeamento voltou a ser literal espalhado".
    for arquivo in ["src/backend_s.rs", "src/backend_text.rs", "src/boot.rs"] {
        let fonte = fs::read_to_string(arquivo).expect("fonte do backend legível");
        // Só o código produtivo: comentários e módulos de teste não decidem
        // identidade de símbolo.
        let codigo: String = fonte
            .split("#[cfg(test)]")
            .next()
            .expect("parte produtiva do arquivo")
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for literal in ["\"principal\"", "\"main\"", "\"_start\""] {
            assert!(
                !codigo.contains(literal),
                "{arquivo} voltou a decidir a identidade do entrypoint por literal {literal}"
            );
        }
    }
}

#[test]
fn superficie_textual_preserva_a_grafia_pinker_deliberadamente() {
    // A diferença entre as duas superfícies é modelada, não acidental.
    assert_eq!(
        native_symbol::function_symbol(
            NativeSurface::Assemblable,
            native_symbol::ENTRYPOINT_SOURCE_IDENTITY
        ),
        native_symbol::ENTRYPOINT_NATIVE_SYMBOL
    );
    assert_eq!(
        native_symbol::function_symbol(
            NativeSurface::TextualAbi,
            native_symbol::ENTRYPOINT_SOURCE_IDENTITY
        ),
        native_symbol::ENTRYPOINT_SOURCE_IDENTITY
    );
}

// ---------------------------------------------------------------------------
// Seção 5 — reserva dirigida do namespace Pinker-owned
// ---------------------------------------------------------------------------

#[test]
fn a_politica_nao_congela_lista_de_libc() {
    // Positivo: nome do host permanece nome Pinker legal em fonte e como
    // definição produtora de símbolo.
    for nome in [
        "malloc", "memcpy", "write", "getenv", "free", "environ", "printf", "exit", "read",
        "strlen", "abort", "qsort",
    ] {
        assert!(
            native_symbol::reserved_namespace(nome, ReservedScope::AnyIdentifier).is_none(),
            "'{nome}' não pode ser reservado: seria blacklist de libc"
        );
        assert!(
            native_symbol::reserved_namespace(nome, ReservedScope::SymbolDefinition).is_none(),
            "'{nome}' não pode ser reservado: seria blacklist de libc"
        );
    }
    // A tabela inteira cobre só espaços realmente possuídos pela Pinker.
    assert_eq!(
        native_symbol::PINKER_OWNED_NAMESPACES.len(),
        4,
        "a reserva é dirigida: compilador, runtime e os dois entrypoints de plataforma"
    );
}

#[test]
fn intrinsecas_publicas_ficam_fora_desta_task() {
    // F-07 é POST_D13_SEPARATE_TASK: esta Task não decide propriedade de nome
    // de intrínseca pública.
    for nome in ["tamanho_verso", "igual_verso", "falar", "ouvir"] {
        assert!(
            native_symbol::reserved_namespace(nome, ReservedScope::AnyIdentifier).is_none(),
            "F-07 fora de escopo: '{nome}' não pode ser reservado aqui"
        );
        assert!(
            native_symbol::reserved_namespace(nome, ReservedScope::SymbolDefinition).is_none(),
            "F-07 fora de escopo: '{nome}' não pode ser reservado aqui"
        );
    }
}

#[test]
fn namespace_do_runtime_e_recusado_cedo() {
    for nome in [
        "pinker_rt_iniciar",
        "pinker_falar_fim",
        "pinker_erro_shift_count",
    ] {
        let fonte = format!(
            "pacote main;\ncarinho {nome}() -> bombom {{ mimo 1; }}\ncarinho principal() -> bombom {{ mimo {nome}(); }}\n"
        );
        let erro =
            checar(&fonte).expect_err(&format!("'{nome}' pertence ao namespace ABI do runtime"));
        assert!(erro.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{erro}");
    }
}

#[test]
fn namespace_do_runtime_produz_diagnostico_pinker() {
    let fonte = "pacote main;\ncarinho pinker_rt_iniciar() -> bombom { mimo 1; }\ncarinho principal() -> bombom { mimo 0; }\n";
    let erro = checar(fonte).expect_err("símbolo do runtime é reservado");
    assert!(erro.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{erro}");
    assert!(erro.contains("libpinker_rt.a"), "{erro}");
}

#[test]
fn namespace_gerado_pelo_compilador_e_recusado_na_fonte() {
    // Representantes reais de cada família de prefixo gerado (F-15).
    for nome in [
        "__pinker_internal_mapa_obter",
        "__anon_carinho_0",
        "__impl_7_Medivel_5_Ponto_medir",
        "__gen_616263",
        "__gen_leque_616263",
        "__fnref_env_somar",
    ] {
        let fonte =
            format!("pacote main;\ncarinho {nome}() -> bombom {{ mimo 1; }}\ncarinho principal() -> bombom {{ mimo 0; }}\n");
        let erro = checar(&fonte).expect_err(&format!("'{nome}' é namespace do compilador"));
        assert!(erro.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{erro}");
    }
}

#[test]
fn todo_prefixo_gerado_vive_sob_o_namespace_reservado() {
    // Sensitivity: se alguém introduzir um prefixo gerado fora de `__`, a
    // reserva dirigida deixaria de cobrir a família nova.
    for prefixo in [
        "__pinker_internal_",
        "__anon_carinho_",
        "__impl_",
        "__trait_default_check_",
        "__gen_",
        "__gen_leque_",
        "__fnref_env_",
        "__fnparam_",
        "__iter_cursor_",
        "__iter_indice_",
        "__iter_lista_",
        "__iter_mapa_",
        "__iter_tamanho_",
        "__propagar_alvo_",
        "__propagar_falha_",
        "__tentar_alvo_",
        "__range_limite_",
        "__ternario",
        "__env",
    ] {
        assert!(
            prefixo.starts_with(native_symbol::COMPILER_GENERATED_PREFIX),
            "prefixo gerado '{prefixo}' fora do namespace reservado"
        );
        assert!(native_symbol::reserved_namespace(prefixo, ReservedScope::AnyIdentifier).is_some());
    }
}

#[test]
fn simbolos_de_boot_de_plataforma_sao_reservados_mas_crt_nao() {
    assert!(
        native_symbol::reserved_namespace("_start", ReservedScope::SymbolDefinition).is_some(),
        "`_start` é produzido pela Pinker no modo livre"
    );
    // `_init`, `_fini` e companhia pertencem ao CRT, não à Pinker: reservá-los
    // seria exatamente a blacklist de host recusada pela Issue.
    for host in ["_init", "_fini", "__libc_start_main", "_edata", "_end"] {
        assert!(
            native_symbol::reserved_namespace(host, ReservedScope::SymbolDefinition).is_none(),
            "'{host}' é do host, não da Pinker"
        );
    }
}

// ---------------------------------------------------------------------------
// Seção 6 — encoding injetivo de labels (F-04)
// ---------------------------------------------------------------------------

/// O caso histórico exato: antes da correção, `f` com laço e `f_loop` com dois
/// `talvez` definiam `.Lf_loop_join_1` duas vezes e o GNU as recusava o
/// programa válido.
#[test]
fn caso_historico_f04_monta_e_executa() {
    const FONTE: &str = r#"
pacote main;

carinho f() -> bombom {
    nova muda i = 0;
    sempre que i < 3 {
        i = i + 1;
    }
    mimo i;
}

carinho f_loop(a: bombom) -> bombom {
    nova muda r = 0;
    talvez a > 1 {
        r = 1;
    }
    talvez a > 2 {
        r = 2;
    }
    mimo r;
}

carinho principal() -> bombom {
    falar(f());
    falar(f_loop(3));
    mimo 0;
}
"#;
    checar(FONTE).expect("programa válido");
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    assert!(
        !asm.contains(".Lf_loop_join_1"),
        "a concatenação ingênua voltou: {asm}"
    );
    assert_eq!(
        rotulos_definidos(&asm).len(),
        rotulos_definidos(&asm)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        "o conjunto de rótulos definidos precisa ser injetivo"
    );

    let Some(program) = build_nativo(concat!(module_path!(), ":", line!()), "f04", FONTE) else {
        return;
    };
    let (codigo, saida) = program.run("issue497-f04");
    assert_eq!((codigo, saida.as_str()), (Some(0), "3\n2\n"));
    assert_eq!(interpretar("f04", FONTE), (Some(0), saida));
}

fn rotulos_definidos(asm: &str) -> Vec<String> {
    asm.lines()
        .map(str::trim)
        .filter(|line| line.starts_with(".L") && line.ends_with(':'))
        .map(|line| line.trim_end_matches(':').to_string())
        .collect()
}

#[test]
fn encoding_de_rotulo_e_injetivo_em_casos_adversariais() {
    // Underscores, números, prefixos, componentes vazios e comprimentos
    // iguais/diferentes. Trocar o separador não produziria esta propriedade.
    let casos: Vec<Vec<&str>> = vec![
        vec!["f", "loop_join_1"],
        vec!["f_loop", "join_1"],
        vec!["a_b", "c"],
        vec!["a", "b_c"],
        vec!["a", "bc"],
        vec!["ab", "c"],
        vec!["", "abc"],
        vec!["abc", ""],
        vec!["a", "", "bc"],
        vec!["a1", "1b"],
        vec!["a", "11_b"],
        vec!["a1", "1_b"],
        vec!["f", "entry"],
        vec!["f_entry", ""],
        vec!["x", "shift_valid", "3"],
        vec!["x_shift", "valid", "3"],
        vec!["x", "shift", "valid_3"],
        vec!["x", "div_done", "12"],
        vec!["x", "div_done_1", "2"],
        vec!["__impl_7_Medivel_5_Ponto_medir", "loop_2"],
    ];
    let mut vistos = BTreeSet::new();
    for componentes in &casos {
        let rotulo = native_symbol::injective_local_label(componentes);
        assert!(
            vistos.insert(rotulo.clone()),
            "componentes distintos colidiram em '{rotulo}': {componentes:?}"
        );
        // A recuperabilidade é a prova, não a improbabilidade.
        assert_eq!(
            native_symbol::decode_injective_local_label(&rotulo)
                .expect("rótulo bem formado")
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            *componentes
        );
    }
}

#[test]
fn classes_reais_de_rotulo_sao_cobertas_pelo_encoding() {
    const FONTE: &str = r#"
pacote main;

carinho classes(a: bombom, b: bombom) -> bombom {
    nova muda total = 0;
    sempre que total < a {
        talvez total > b {
            quebrar;
        } senao {
            total = total + 1;
        }
    }
    nova d: bombom = a / (b + 1);
    nova s: bombom = a << (b + 1);
    mimo total + d + s;
}

carinho principal() -> bombom {
    falar(classes(4, 1));
    mimo 0;
}
"#;
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    let rotulos = rotulos_definidos(&asm);
    assert!(!rotulos.is_empty());
    for rotulo in &rotulos {
        if rotulo.starts_with(native_symbol::INJECTIVE_LOCAL_LABEL_PREFIX) {
            assert!(
                native_symbol::decode_injective_local_label(rotulo).is_some(),
                "rótulo '{rotulo}' não é recuperável pela autoridade"
            );
        } else {
            // As demais classes já eram injetivas por construção própria
            // (namespace `.Lpinker_*` com encoding hex delimitado).
            assert!(
                rotulo.starts_with(".Lpinker_"),
                "rótulo '{rotulo}' fora das classes cobertas"
            );
        }
    }
    let unicos: BTreeSet<&String> = rotulos.iter().collect();
    assert_eq!(rotulos.len(), unicos.len(), "rótulos duplicados em {asm}");
}

// ---------------------------------------------------------------------------
// Seção 12 — injetividade do conjunto emitido
// ---------------------------------------------------------------------------

#[test]
fn conjunto_emitido_nao_tem_duas_identidades_no_mesmo_simbolo() {
    const FONTE: &str = r#"
pacote main;

eterno LIMITE: bombom = 3;

carinho dobro(n: bombom) -> bombom {
    mimo n * 2;
}

carinho triplo(n: bombom) -> bombom {
    nova muda total = 0;
    nova muda i = 0;
    sempre que i < 3 {
        total = total + n;
        i = i + 1;
    }
    mimo total;
}

carinho principal() -> bombom {
    falar(dobro(LIMITE) + triplo(LIMITE));
    mimo 0;
}
"#;
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    let mut definicoes: Vec<String> = rotulos_definidos(&asm);
    definicoes.extend(
        asm.lines()
            .map(str::trim)
            .filter(|line| line.ends_with(':') && !line.starts_with('.') && !line.contains(' '))
            .map(|line| line.trim_end_matches(':').to_string()),
    );
    let unicas: BTreeSet<&String> = definicoes.iter().collect();
    assert_eq!(
        definicoes.len(),
        unicas.len(),
        "duas definições emitidas colidiram:\n{asm}"
    );
}

#[test]
fn muitos_para_um_deliberado_nao_vira_bug() {
    // A mesma identidade pode renderizar para o mesmo símbolo quantas vezes
    // for necessário; só identidades distintas no mesmo símbolo são colisão.
    let mut conjunto = native_symbol::EmittedDefinitions::new();
    conjunto.define("pinker_falar_fim", "intrínseca falar");
    conjunto.define("pinker_falar_fim", "intrínseca falar");
    assert!(conjunto.first_collision().is_none());
    conjunto.define("pinker_falar_fim", "carinho do usuário");
    assert!(conjunto.first_collision().is_some());
}

#[test]
fn diagnostico_de_colisao_nao_depende_de_ordem_de_hashmap() {
    let mensagem = |ordem: [(&str, &str); 3]| {
        let mut conjunto = native_symbol::EmittedDefinitions::new();
        for (simbolo, identidade) in ordem {
            conjunto.define(simbolo, identidade);
        }
        conjunto
            .first_collision()
            .map(native_symbol::emitted_collision_message)
    };
    let direto = mensagem([("z", "a"), ("z", "b"), ("z", "c")]);
    let invertido = mensagem([("z", "a"), ("z", "c"), ("z", "b")]);
    assert!(direto.is_some());
    assert_eq!(
        direto, invertido,
        "o diagnóstico precisa ser determinístico"
    );
}

// ---------------------------------------------------------------------------
// Seção 8 — `sussurro` (D4) preservado
// ---------------------------------------------------------------------------

#[test]
fn sussurro_continua_verde_no_caminho_real() {
    const FONTE: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova antes: bombom = 20;
    sussurro(
        "nop",
        "1:\n  nop\n  jmp 2f\n  nop\n2:"
    );
    nova depois: bombom = 22;
    falar(antes + depois);
    mimo 0;
}
"#;
    let Some(program) = build_nativo(
        concat!(module_path!(), ":", line!()),
        "sussurro_envelope",
        FONTE,
    ) else {
        return;
    };
    let (codigo, saida) = program.run("issue497-sussurro");
    assert_eq!((codigo, saida.as_str()), (Some(0), "42\n"));

    // A função que carrega envelope é a única classe sem `.size`: o invariante
    // de artefato de D4 proíbe o delta de superfície que o envelope produz.
    assert!(
        program.assembly.contains(".globl main"),
        "{}",
        program.assembly
    );
    assert!(
        program.assembly.contains(".type main, @function"),
        "{}",
        program.assembly
    );
    assert!(
        !program.assembly.contains(".size main"),
        "declarar `.size` numa função com envelope abortaria o build por E-BACKEND-ASM-ARTIFACT"
    );
}

#[test]
fn sussurro_por_operando_continua_aceito_com_entrypoint_e_ligacao_novos() {
    const FONTE: &str = r#"
pacote main;

carinho auxiliar(n: bombom) -> bombom {
    mimo n + 1;
}

carinho principal() -> bombom {
    nova a: bombom = 20;
    nova b: bombom = 21;
    nova muda resultado: bombom = 0;
    sussurro(
        "mov {resultado}, {a}\nadd {resultado}, {b}";
        entrada a: r8 = a;
        entrada b: r9 = b;
        saida resultado: r11 = resultado;
        destroi(flags)
    );
    falar(auxiliar(resultado));
    mimo 0;
}
"#;
    checar(FONTE).expect("D4 não é reaberta: o programa continua aceito");
    let Some(program) = build_nativo(
        concat!(module_path!(), ":", line!()),
        "sussurro_operandos",
        FONTE,
    ) else {
        return;
    };
    let (codigo, saida) = program.run("issue497-sussurro-operandos");
    assert_eq!((codigo, saida.as_str()), (Some(0), "42\n"));
    assert_eq!(interpretar("sussurro_operandos", FONTE).0, Some(1));

    // A função sem envelope no mesmo programa continua recebendo `.size`.
    let auxiliar = program.defined("auxiliar").expect("auxiliar definida");
    assert_eq!(auxiliar.bind, "LOCAL");
    assert!(auxiliar.size > 0);
}

// ---------------------------------------------------------------------------
// Seção 10 — sensitivity contra as mutações óbvias
// ---------------------------------------------------------------------------

#[test]
fn sensitivity_a_emissao_usa_local_e_nunca_hidden() {
    const FONTE: &str = r#"
pacote main;

eterno G: bombom = 1;

carinho aux() -> bombom {
    mimo G;
}

carinho principal() -> bombom {
    falar(aux());
    mimo 0;
}
"#;
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    assert!(asm.contains(".local aux"), "{asm}");
    assert!(asm.contains(".local G"), "{asm}");
    assert!(asm.contains(".globl main"), "{asm}");
    assert!(
        !asm.contains(".hidden"),
        "STV_HIDDEN não corrige captura em link estático: {asm}"
    );
    assert!(
        !asm.contains(".globl aux") && !asm.contains(".globl G"),
        "{asm}"
    );
    assert_eq!(
        native_symbol::NativeBinding::Local.directive("aux"),
        ".local aux"
    );
}

#[test]
fn sensitivity_a_ligacao_por_classe() {
    assert_eq!(
        native_symbol::function_binding(native_symbol::ENTRYPOINT_SOURCE_IDENTITY),
        NativeBinding::Global,
        "main local quebraria o CRT"
    );
    for nome in ["malloc", "auxiliar", "__impl_7_T_5_X_m", "__anon_carinho_0"] {
        assert_eq!(native_symbol::function_binding(nome), NativeBinding::Local);
    }
    assert_eq!(
        native_symbol::native_binding(NativeDefinition::UserGlobal),
        NativeBinding::Local
    );
    assert_eq!(
        native_symbol::classify_function("__gen_616263"),
        NativeDefinition::GeneratedFunction,
        "função gerada continua global seria regressão"
    );
}

#[test]
fn sensitivity_nenhum_gc_sections_no_link_do_produto() {
    let fonte = fs::read_to_string("src/main.rs").expect("CLI legível");
    assert!(
        !fonte.contains("gc-sections"),
        "--gc-sections não é correção de captura de símbolo"
    );
}

#[test]
fn sensitivity_nenhum_mangling_geral_de_nome_de_usuario() {
    const FONTE: &str = r#"
pacote main;

carinho soma_visivel(a: bombom) -> bombom {
    mimo a;
}

carinho principal() -> bombom {
    falar(soma_visivel(42));
    mimo 0;
}
"#;
    let asm = common::render_backend_s_external_subset_nativo(FONTE).expect("assembly nativo");
    assert!(
        asm.contains("soma_visivel:"),
        "o nome do usuário continua sendo o símbolo, sem mangling geral: {asm}"
    );
}
