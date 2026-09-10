//! U-01 / TC-01 — guard da autoridade única do contrato de operações internas.
//!
//! O que esta suíte protege não é a existência de um registro: é a propriedade
//! de que **uma** mudança num fato compartilhado de operação interna exige
//! mudar **uma** autoridade.
//!
//! ```text
//! CHANGE_ONE_SHARED_INTERNAL_OPERATION_FACT
//! -> CHANGE_ONE_CANONICAL_AUTHORITY
//! ```
//!
//! A pergunta obrigatória — *este guard poderia continuar verde se um
//! consumidor reimplementasse localmente o mesmo contrato?* — é respondida por
//! duas metades que só juntas fecham o caminho:
//!
//! 1. **Exaustividade** (`LAW-01`): o conjunto declarado é exatamente o
//!    conjunto usado por `src/**`. Acrescentar uma grafia interna sem declarar,
//!    ou declarar uma sem produtor/consumidor real, fica vermelho.
//! 2. **Ausência de segunda decisão**: varrendo `src/**` inteiro, com uma
//!    allowlist explícita de PAPÉIS — autoridade, identidade, corpos do
//!    interpretador e produtores —, nenhum derivador pode declarar contrato ao
//!    lado da operação (tipo IR, tipo de pilha, símbolo de runtime) nem
//!    responder aridade sem perguntar à autoridade. A regra é sobre o fato, não
//!    sobre a forma: `insert`, `match`, `if`, array e comparação caem todos, o
//!    lado direito da aridade precisa ser consulta à autoridade — literal,
//!    constante nomeada ou expressão não servem — e trocar `!=` por `<` não
//!    ajuda. Como o escopo é a árvore, extrair a segunda tabela para um arquivo
//!    novo não a esconde: torna esse arquivo um derivador não isento.
//!
//! As fronteiras que U-01 não pode atravessar também são verificadas aqui:
//! superfície pública (C1), símbolo ABI, identidade de usuário e a relação de
//! especialização de mapa que pertence a U-02.

use pinker_v0::internal_operations::{self, InternalOperationFamily, INTERNAL_OPERATIONS};
use pinker_v0::intrinsics::identity::{callee_identity_de_ident, CalleeIdentity};
use pinker_v0::intrinsics::registry;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const AUTHORITY_FILE: &str = "src/internal_operations.rs";

/// Papéis que a #651 distingue e que PODEM nomear uma operação interna junto
/// com contrato ou aridade. Tudo o mais em `src/**` é derivador.
///
/// A lista é de EXCEÇÕES, não de vigiados: o escopo do guard é a árvore
/// inteira. Extrair a segunda tabela para um arquivo novo ao lado não a torna
/// invisível — torna-a um derivador não isento, e o guard fecha nele.
const PAPEIS_ISENTOS: &[&str] = &[
    // A autoridade canônica.
    "src/internal_operations.rs",
    // Autoridade das grafias de carga de leque e da classificação de carga.
    "src/enum_payload.rs",
    // Autoridade de identidade: reserva de namespace e classe do callee.
    "src/native_symbol.rs",
    "src/intrinsics/identity.rs",
    // INTERPRETER_BODY: a #651 proíbe mover estes corpos para a autoridade.
    "src/interpreter.rs",
    "src/interpreter/hosted_intrinsics.rs",
    // Produtores: decidem QUANDO emitir, não O QUE a operação é.
    "src/parser/lacos.rs",
    "src/parser/resultado.rs",
    "src/parser/expressoes.rs",
    "src/ir/lowering.rs",
    "src/cfg_ir.rs",
];

/// Marcadores de que o texto seguinte está num ramo já decidido pela
/// autoridade. Depois deles, responder aridade por conta própria é decisão
/// local, qualquer que seja a forma do lado direito.
const MARCADORES_DE_RAMO_DA_AUTORIDADE: &[&str] = &[
    "internal_operations::e_ternaria(",
    "internal_operations::e_operacao_generica_de_mapa(",
    "internal_operations::entrada(",
    "internal_operations::aridade(",
    "aridade_interna(",
    "is_generic_map_intrinsic(",
];

/// Tokens que só aparecem quando alguém está declarando contrato estrutural.
const TOKENS_DE_CONTRATO: &[&str] = &["TypeIR::", "StackValueType::", "\"pinker_"];

/// Formas de perguntar a aridade à autoridade. Um lado direito que não seja
/// uma delas é resposta local, seja literal, constante nomeada ou expressão.
const RESPOSTAS_DA_AUTORIDADE: &[&str] = &[
    "internal_operations::aridade",
    "aridade_interna(",
    ".arity()",
    "operation.arity",
];

/// Comparações de aridade. Todas contam: trocar `!=` por `<`/`>` não muda o
/// fato decidido.
const OPERADORES_DE_COMPARACAO: &[&str] = &["==", "!=", "<=", ">=", "<", ">"];

/// A janela é a checagem cruzada de classe de carga, e não uma declaração?
///
/// `ir_validate` compara a representação operacional de uma carga de leque
/// contra a classe declarada por `enum_payload`. É a inversa que aquele
/// validador existe para fazer, e o marcador dela é `EnumPayloadClass::` — não
/// o mero fato de o tipo estar à direita de `==`, que é exatamente como um
/// validador escreveria um contrato local.
fn e_cruzamento_com_enum_payload(janela: &str) -> bool {
    janela.contains("EnumPayloadClass::")
}

/// Fatia segura em fronteira de caractere, à frente de `inicio`.
fn janela_a_frente(texto: &str, inicio: usize, bytes: usize) -> &str {
    let mut fim = (inicio + bytes).min(texto.len());
    while fim > inicio && !texto.is_char_boundary(fim) {
        fim -= 1;
    }
    &texto[inicio..fim]
}

/// Fatia segura em fronteira de caractere, atrás de `fim`.
fn janela_atras(texto: &str, fim: usize, bytes: usize) -> &str {
    let mut inicio = fim.saturating_sub(bytes);
    while inicio < fim && !texto.is_char_boundary(inicio) {
        inicio += 1;
    }
    &texto[inicio..fim]
}

/// Posições de toda menção a uma operação interna: grafia literal ou constante
/// de `enum_payload`, que é onde as grafias de leque são declaradas.
fn mencoes_de_operacao_interna(texto: &str) -> Vec<usize> {
    let mut posicoes = Vec::new();
    for marcador in [
        "\"__pinker_internal_",
        "\"__ternario\"",
        "enum_payload::ANEXAR",
        "enum_payload::CARGA",
    ] {
        let mut base = 0usize;
        while let Some(deslocamento) = texto[base..].find(marcador) {
            posicoes.push(base + deslocamento);
            base += deslocamento + marcador.len();
        }
    }
    posicoes.sort_unstable();
    posicoes
}

/// Um número solto logo depois de nomear a operação é resposta local.
fn responde_com_numero_solto(janela: &str) -> bool {
    let bytes = janela.as_bytes();
    for (indice, _) in janela.match_indices("=>") {
        let mut cursor = indice + 2;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if janela[cursor..].starts_with("return") {
            cursor += "return".len();
            while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                cursor += 1;
            }
        }
        if bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            return true;
        }
    }
    false
}

/// Aridade respondida sem perguntar à autoridade.
///
/// Não procura "número mágico": procura um lado direito que NÃO seja consulta à
/// autoridade. Constante nomeada, expressão e literal caem todos aqui, e trocar
/// o operador não ajuda.
fn responde_aridade_sem_a_autoridade(janela: &str) -> bool {
    let bytes = janela.as_bytes();
    for alvo in [".len()", "argc"] {
        for (indice, _) in janela.match_indices(alvo) {
            let mut cursor = indice + alvo.len();
            while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                cursor += 1;
            }
            let Some(operador) = OPERADORES_DE_COMPARACAO
                .iter()
                .find(|operador| janela[cursor..].starts_with(*operador))
            else {
                continue;
            };
            cursor += operador.len();
            // Lado direito até o fim da subexpressão.
            let direito = &janela[cursor..];
            let fim = direito
                .find(|c| c == '{' || c == ';')
                .unwrap_or(direito.len());
            let direito = direito[..fim]
                .split("&&")
                .next()
                .unwrap_or("")
                .split("||")
                .next()
                .unwrap_or("");
            if !RESPOSTAS_DA_AUTORIDADE
                .iter()
                .any(|resposta| direito.contains(*resposta))
            {
                return true;
            }
        }
    }
    false
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fontes_rust(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("ler diretório de fontes") {
        let path = entry.expect("entrada de diretório").path();
        if path.is_dir() {
            fontes_rust(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Grafias internas literais de um texto Rust.
///
/// O prefixo cru `"__pinker_internal_"` — a constante de namespace e o
/// `format!` do lowering — não é grafia de operação e não entra.
fn grafias_literais(texto: &str) -> BTreeSet<String> {
    let mut encontradas = BTreeSet::new();
    for (marcador, sufixo_obrigatorio) in
        [("\"__pinker_internal_", true), ("\"__ternario\"", false)]
    {
        let mut resto = texto;
        while let Some(pos) = resto.find(marcador) {
            resto = &resto[pos + 1..];
            let Some(fim) = resto.find('"') else { break };
            let grafia = &resto[..fim];
            // `format!("__pinker_internal_{name}")` é composição do produtor,
            // não grafia: as cinco formas que ele compõe aparecem literalmente
            // no corpo do interpretador e entram por lá.
            if grafia.contains('{') {
                continue;
            }
            if !sufixo_obrigatorio || grafia.len() > "__pinker_internal_".len() {
                encontradas.insert(grafia.to_string());
            }
        }
    }
    encontradas
}

fn declaradas() -> BTreeSet<String> {
    INTERNAL_OPERATIONS
        .iter()
        .map(|operation| operation.spelling.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Exaustividade — LAW-01
// ---------------------------------------------------------------------------

#[test]
fn law_01_toda_grafia_interna_usada_esta_declarada_na_autoridade() {
    let raiz = repo();
    let mut fontes = Vec::new();
    fontes_rust(&raiz.join("src"), &mut fontes);

    let declaradas = declaradas();
    let mut nao_declaradas: Vec<(String, String)> = Vec::new();
    for fonte in &fontes {
        let relativo = fonte.strip_prefix(&raiz).expect("caminho relativo");
        if relativo == Path::new(AUTHORITY_FILE) {
            continue;
        }
        let texto = std::fs::read_to_string(fonte).expect("ler fonte");
        for grafia in grafias_literais(&texto) {
            if !declaradas.contains(&grafia) {
                nao_declaradas.push((relativo.display().to_string(), grafia));
            }
        }
    }
    assert!(
        nao_declaradas.is_empty(),
        "grafias internas usadas por src/** sem contrato declarado na autoridade: {nao_declaradas:?}"
    );
}

#[test]
fn law_01_toda_grafia_declarada_tem_produtor_ou_consumidor_real() {
    let raiz = repo();
    let mut fontes = Vec::new();
    fontes_rust(&raiz.join("src"), &mut fontes);

    let mut usadas: BTreeSet<String> = BTreeSet::new();
    for fonte in &fontes {
        let relativo = fonte.strip_prefix(&raiz).expect("caminho relativo");
        if relativo == Path::new(AUTHORITY_FILE) {
            continue;
        }
        let texto = std::fs::read_to_string(fonte).expect("ler fonte");
        usadas.extend(grafias_literais(&texto));
    }

    let decorativas: Vec<String> = declaradas()
        .into_iter()
        .filter(|grafia| !usadas.contains(grafia))
        .collect();
    assert!(
        decorativas.is_empty(),
        "entradas declarativas sem produtor nem consumidor em src/**: {decorativas:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. Nenhuma segunda tabela de contrato pode voltar
// ---------------------------------------------------------------------------

/// Todo arquivo de `src/**` que não exerce um papel isento.
fn derivadores(raiz: &Path) -> Vec<(String, String)> {
    let mut fontes = Vec::new();
    fontes_rust(&raiz.join("src"), &mut fontes);
    fontes
        .into_iter()
        .filter_map(|caminho| {
            let relativo = caminho
                .strip_prefix(raiz)
                .expect("caminho relativo")
                .to_string_lossy()
                .into_owned();
            if PAPEIS_ISENTOS.contains(&relativo.as_str()) {
                return None;
            }
            let texto = std::fs::read_to_string(&caminho).expect("ler fonte");
            Some((relativo, texto))
        })
        .collect()
}

#[test]
fn nenhum_derivador_declara_contrato_ao_lado_da_grafia_interna() {
    // Generaliza a forma física: não é só `insert(` numa tabela. Nomear a
    // operação e, na vizinhança, escrever tipo IR, tipo de pilha ou símbolo de
    // runtime é declarar contrato local — em `insert`, `match`, `if`, array ou
    // comparação.
    let raiz = repo();
    let mut ofensores = Vec::new();
    for (derivador, texto) in derivadores(&raiz) {
        for posicao in mencoes_de_operacao_interna(&texto) {
            let janela = janela_a_frente(&texto, posicao, 200);
            // `builtin_nominal_sig` não é contrato estrutural: é a identidade
            // NOMINAL de `falha_operacional`, que continua com o dono dela.
            if janela.contains("builtin_nominal_sig") || e_cruzamento_com_enum_payload(janela) {
                continue;
            }
            for token in TOKENS_DE_CONTRATO {
                if janela.contains(token) {
                    ofensores.push(format!(
                        "{derivador}: byte {posicao} nomeia operação interna e declara '{token}'"
                    ));
                }
            }
        }
        // O símbolo também pode ser escrito ANTES da grafia.
        for (posicao, _) in texto.match_indices("\"pinker_") {
            let janela = janela_atras(&texto, posicao, 200);
            if !mencoes_de_operacao_interna(janela).is_empty()
                && !e_cruzamento_com_enum_payload(janela)
            {
                ofensores.push(format!(
                    "{derivador}: byte {posicao} roteia símbolo por operação interna"
                ));
            }
        }
    }
    assert!(
        ofensores.is_empty(),
        "contrato estrutural local por operação interna reintroduzido: {ofensores:?}"
    );
}

#[test]
fn nenhum_derivador_responde_aridade_sem_perguntar_a_autoridade() {
    // Fecha a forma que não nomeia a grafia: dentro de um ramo que a autoridade
    // já decidiu, responder aridade por conta própria é reimplementar o
    // contrato — com literal, com constante nomeada ou com qualquer operador.
    let raiz = repo();
    let mut ofensores = Vec::new();
    for (derivador, texto) in derivadores(&raiz) {
        for marcador in MARCADORES_DE_RAMO_DA_AUTORIDADE {
            for (posicao, _) in texto.match_indices(marcador) {
                let janela = janela_a_frente(&texto, posicao, 260);
                if responde_aridade_sem_a_autoridade(janela) {
                    ofensores.push(format!(
                        "{derivador}: byte {posicao} responde aridade sem a autoridade dentro do ramo dela"
                    ));
                }
            }
        }
        // E as duas formas que nomeiam a grafia: número solto, ou aridade
        // respondida localmente logo ao lado da menção.
        for posicao in mencoes_de_operacao_interna(&texto) {
            if responde_com_numero_solto(janela_a_frente(&texto, posicao, 120)) {
                ofensores.push(format!(
                    "{derivador}: byte {posicao} devolve número solto para operação interna"
                ));
            }
            let janela = janela_a_frente(&texto, posicao, 200);
            if !e_cruzamento_com_enum_payload(janela) && responde_aridade_sem_a_autoridade(janela) {
                ofensores.push(format!(
                    "{derivador}: byte {posicao} decide aridade ao lado da grafia interna"
                ));
            }
        }
    }
    assert!(
        ofensores.is_empty(),
        "aridade local reintroduzida: {ofensores:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Fronteiras que U-01 não atravessa
// ---------------------------------------------------------------------------

#[test]
fn simbolo_abi_nunca_e_identidade_de_operacao_interna() {
    for operation in INTERNAL_OPERATIONS {
        let Some(simbolo) = operation.runtime_symbol else {
            continue;
        };
        assert!(
            simbolo.starts_with("pinker_"),
            "símbolo de runtime fora do namespace ABI: {simbolo}"
        );
        assert!(
            !internal_operations::e_operacao_interna(simbolo),
            "o símbolo ABI '{simbolo}' foi aceito como identidade interna"
        );
        assert_ne!(
            operation.spelling, simbolo,
            "identidade interna colapsou no símbolo ABI"
        );
    }
}

#[test]
fn superficie_publica_c1_e_a_interna_nao_se_tocam() {
    for operation in INTERNAL_OPERATIONS {
        assert!(
            !registry::e_historica(operation.spelling),
            "grafia interna '{}' entrou no registry público",
            operation.spelling
        );
        assert_eq!(
            pinker_v0::intrinsics::identity::intrinsic_from_public_spelling(operation.spelling),
            None,
            "grafia interna '{}' virou intrínseca pública",
            operation.spelling
        );
    }
    for grafia in registry::grafias() {
        assert!(
            !internal_operations::e_operacao_interna(grafia),
            "grafia pública '{grafia}' entrou na autoridade interna"
        );
    }
}

#[test]
fn callee_de_usuario_nunca_vira_operacao_interna() {
    // `__usuario` compartilha o superprefixo `__` e continua sendo do usuário.
    assert_eq!(callee_identity_de_ident("__usuario"), CalleeIdentity::User);
    assert!(!internal_operations::e_operacao_interna("__usuario"));
    assert_eq!(callee_identity_de_ident("principal"), CalleeIdentity::User);

    // A classe continua vindo de `native_symbol`, não desta autoridade.
    for operation in INTERNAL_OPERATIONS {
        assert_eq!(
            callee_identity_de_ident(operation.spelling),
            CalleeIdentity::CompilerInternal,
            "operação interna '{}' perdeu a classe CompilerInternal",
            operation.spelling
        );
    }
}

/// Alvos declarados por uma tabela de especialização de mapa, lidos da FONTE.
///
/// Ler a fonte, e não reconstruir os nomes por `format!`, é o que faz este
/// teste detectar uma mudança do lado direito da relação — que é exatamente o
/// que U-02 faria.
fn alvos_da_tabela_de_especializacao(texto: &str, funcao: &str) -> Vec<String> {
    let inicio = texto
        .find(funcao)
        .unwrap_or_else(|| panic!("tabela de especialização '{funcao}' desapareceu"));
    let corpo = &texto[inicio..];
    let fim = corpo
        .find("_ => None,")
        .expect("tabela de especialização sem braço final");
    let mut alvos = Vec::new();
    let mut resto = &corpo[..fim];
    while let Some(abre) = resto.find("Some(\"") {
        resto = &resto[abre + "Some(\"".len()..];
        let fecha = resto.find('"').expect("literal sem fechamento");
        alvos.push(resto[..fecha].to_string());
        resto = &resto[fecha..];
    }
    alvos
}

#[test]
fn a_autoridade_nao_absorve_a_relacao_de_especializacao_de_mapa_de_u02() {
    // U-02 é `(classe concreta de mapa, operação genérica) -> grafia
    // monomórfica`. As três realizações paralelas dessa relação continuam onde
    // estão, e os alvos delas são grafias HISTÓRICAS do registry público.
    let raiz = repo();
    let tabelas = [
        ("src/ir.rs", "fn generic_map_monomorphic_callee"),
        ("src/parser/mod.rs", "fn generic_map_callee"),
        ("src/semantic/calls.rs", "fn generic_map_monomorphic_callee"),
    ];
    let mut conjuntos = Vec::new();
    for (arquivo, funcao) in tabelas {
        let texto = std::fs::read_to_string(raiz.join(arquivo)).expect("ler tabela de U-02");
        let alvos = alvos_da_tabela_de_especializacao(&texto, funcao);
        assert_eq!(
            alvos.len(),
            20,
            "{arquivo}: a relação de U-02 deixou de ter 20 combinações"
        );
        for alvo in &alvos {
            assert!(
                registry::e_historica(alvo),
                "{arquivo}: '{alvo}' saiu do registry histórico"
            );
            assert!(
                !internal_operations::e_operacao_interna(alvo),
                "U-02 absorvida: {arquivo} passou a resolver para a operação interna '{alvo}'"
            );
        }
        conjuntos.push(alvos);
    }
    assert!(
        conjuntos.windows(2).all(|par| par[0] == par[1]),
        "as três realizações da relação de U-02 deixaram de concordar"
    );
    // E nenhuma grafia monomórfica de mapa entrou na autoridade interna.
    for operation in INTERNAL_OPERATIONS {
        assert!(
            !registry::e_historica(operation.spelling),
            "grafia histórica '{}' entrou na autoridade interna",
            operation.spelling
        );
    }
}

// ---------------------------------------------------------------------------
// 4. O contrato declarado é o contrato que as fases usam
// ---------------------------------------------------------------------------

#[test]
fn o_contrato_declarado_e_coerente_consigo_mesmo() {
    for operation in INTERNAL_OPERATIONS {
        // Aridade e parâmetros nunca discordam.
        if let Some(params) = operation.declared_params() {
            assert_eq!(params.len(), operation.arity(), "{}", operation.spelling);
        }
        // Só a família genérica de mapa tem contrato relativo ao receptor.
        let relativo = operation.declared_params().is_none()
            && operation.family != InternalOperationFamily::Ternaria;
        if relativo {
            assert_eq!(
                operation.family,
                InternalOperationFamily::MapaGenerica,
                "contrato relativo fora da família genérica: {}",
                operation.spelling
            );
        }
        // A consulta pela grafia devolve a própria entrada.
        assert_eq!(
            internal_operations::entrada(operation.spelling),
            Some(operation),
            "{}",
            operation.spelling
        );
        assert_eq!(
            internal_operations::aridade(operation.spelling),
            Some(operation.arity())
        );
    }
}

#[test]
fn a_ternaria_esta_na_autoridade_e_nao_e_reconhecida_por_prefixo_textual() {
    // Prova de que a autoridade não é "o prefixo `__pinker_internal_`".
    assert!(internal_operations::e_ternaria("__ternario"));
    assert!(internal_operations::e_operacao_interna("__ternario"));
    assert_eq!(internal_operations::aridade("__ternario"), Some(3));
    assert_eq!(internal_operations::simbolo_runtime("__ternario"), None);
    // E que um nome do mesmo formato, mas inexistente, não é aceito.
    assert!(!internal_operations::e_operacao_interna("__ternariox"));
    assert!(!internal_operations::e_operacao_interna(
        "__pinker_internal_mapa_inexistente"
    ));
}

#[test]
fn as_operacoes_sem_valor_sao_exatamente_as_declaradas_como_nulo() {
    let sem_valor: Vec<&str> = INTERNAL_OPERATIONS
        .iter()
        .filter(|operation| !operation.returns_value())
        .map(|operation| operation.spelling)
        .collect();
    assert_eq!(
        sem_valor,
        vec![
            "__pinker_internal_mapa_definir",
            "__pinker_internal_mapa_remover",
        ]
    );
}
