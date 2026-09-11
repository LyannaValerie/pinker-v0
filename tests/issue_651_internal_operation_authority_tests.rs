//! U-01 / TC-01 — fronteiras e exaustividade da autoridade de operações
//! internas do compilador.
//!
//! ```text
//! CHANGE_ONE_SHARED_INTERNAL_OPERATION_FACT
//! -> CHANGE_ONE_CANONICAL_AUTHORITY
//! ```
//!
//! A pergunta obrigatória — *este guard poderia continuar verde se um
//! consumidor reimplementasse localmente o mesmo contrato?* — **não** é
//! respondida aqui, e esta suíte não afirma em lugar nenhum que responde.
//!
//! Ela é respondida por EXECUÇÃO, em
//! `src/internal_operations/metamorphic_oracle.rs`, e nos DOIS sentidos: mutar
//! um fato da autoridade canônica para ESTREITAR, e exigir que todo consumidor
//! que decide aquele fato mude a resposta junto; e mutar para ADMITIR um caso
//! novo, exibindo uma testemunha que só o contrato mutado aceita, e exigir que
//! o mesmo consumidor passe a aceitá-la. A primeira metade pega a decisão local
//! que SOMBREIA a autoridade; a segunda pega a que roda ADITIVA e CONCORDE ao
//! lado dela. Nenhuma das duas olha o texto: arquivo, posição, helper, macro,
//! operador e alias não mudam o veredito.
//!
//! ```text
//! AUTHORITY_PROOF   =
//! BOUNDED_BIDIRECTIONAL_SEMANTIC_METAMORPHIC_EXECUTION
//! SOURCE_SHAPE_LINT = SUPPLEMENTAL_ONLY
//! ```
//!
//! **Limitada** não é ressalva de estilo: é o escopo medido. A prova vale sobre
//! o domínio declarado em [`INTERNAL_OPERATIONS`], sobre as dimensões
//! identificadas de cada fato, sobre os consumidores que três descobertas
//! independentes encontram, e sobre as classes materiais de duplicação que ela
//! nomeia. Ela NÃO afirma que nenhum trecho de Rust arbitrário, em lugar
//! nenhum da árvore, possa responder à mesma pergunta. O domínio exato, e as
//! classes de disposição de cada obrigação direcional, estão no cabeçalho
//! daquele módulo.
//!
//! O fechamento dirigido do HEAD `f8683d87` mediu o limite do caminho
//! sintático: de 21 reimplementações locais do MESMO contrato, 19 compilaram e
//! passaram por estas regras mudando apenas macro, aritmética, padrão de
//! fatia, helper ou arquivo hospedeiro. Decidir se um trecho de Rust
//! arbitrário responde à mesma pergunta é indecidível por inspeção léxica.
//!
//! O que esta suíte prova, e continua sound:
//!
//! 1. **Exaustividade** (`LAW-01`): o conjunto declarado é exatamente o
//!    conjunto de grafias internas usadas por `src/**`. Acrescentar uma grafia
//!    interna sem declarar, ou declarar uma sem produtor/consumidor real, fica
//!    vermelho.
//! 2. **Fronteiras que U-01 não pode atravessar**: superfície pública (C1),
//!    símbolo ABI, identidade de usuário, a relação de especialização de mapa
//!    que pertence a U-02 e a coerência interna do contrato declarado.
//! 3. **Guard estrutural suplementar**: as regras léxicas que sobraram são
//!    regression guard barato contra as formas JÁ VISTAS, e só isso. Elas
//!    levam `guard_suplementar_` no nome exatamente para que ninguém volte a
//!    lê-las como prova de autoridade única.
//!
//! ```text
//! SUPPLEMENTAL_STRUCTURAL_GUARD
//! NOT_SEMANTIC_AUTHORITY_PROOF
//! ```

use pinker_v0::internal_operations::{self, InternalOperationFamily, INTERNAL_OPERATIONS};
use pinker_v0::intrinsics::identity::{callee_identity_de_ident, CalleeIdentity};
use pinker_v0::intrinsics::registry;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const AUTHORITY_FILE: &str = "src/internal_operations.rs";

/// O harness metamórfico, submódulo da autoridade.
const ORACULO_FILE: &str = "src/internal_operations/metamorphic_oracle.rs";

/// A ÚNICA grafia interna de `src/**` que não está na autoridade.
///
/// É a entrada SINTÉTICA da prova permissiva de pertinência: ela existe apenas
/// dentro de uma variante instalada por teste, sob `cfg(test)`, e nunca em
/// [`INTERNAL_OPERATIONS`]. Declará-la na autoridade criaria operação de
/// produção — com superfície, binding e ABI que ela não pode ter. Deixá-la
/// passar calada abriria a LAW-01 para qualquer grafia nova no harness. A
/// exceção é nominal e vale só neste arquivo.
const GRAFIA_SINTETICA_DO_ORACULO: &str = "__pinker_internal_u01_pertinencia_sintetica";

/// O oráculo metamórfico: harness `#[cfg(test)]` da própria autoridade.
///
/// Ele nomeia grafias e contratos porque é o que MUTA o contrato canônico para
/// provar que todo consumidor acompanha. Não é derivador de fase, e por isso
/// não entra no guard estrutural suplementar.
const ORACLE_FILE: &str = "src/internal_operations/metamorphic_oracle.rs";

/// Papéis que a #651 distingue e que PODEM nomear uma operação interna junto
/// com contrato ou aridade. Tudo o mais em `src/**` é derivador.
///
/// A lista é de EXCEÇÕES, não de vigiados: o escopo do guard é a árvore
/// inteira. Extrair a segunda tabela para um arquivo novo ao lado não a torna
/// invisível — torna-a um derivador não isento, e o guard fecha nele.
const PAPEIS_ISENTOS: &[&str] = &[
    // A autoridade canônica.
    AUTHORITY_FILE,
    ORACLE_FILE,
    // Autoridade das grafias de carga de leque e da classificação de carga.
    "src/enum_payload.rs",
    // BINDING_NATIVO: dono único preexistente da relação `operação interna ->
    // símbolo do runtime`. A #651 só autorizaria consolidar essa relação se
    // várias fases a repetissem; o reinventário provou que não repetiam, então
    // ela ficou onde estava (G651-01).
    "src/backend_s.rs",
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

/// O lado direito é uma consulta à autoridade?
///
/// `.arity()` sozinho não basta: o receptor tem de vir da autoridade. Um método
/// `arity` de outro objeto responde a mesma pergunta por conta própria, e é
/// decisão local.
fn e_consulta_a_autoridade(direito: &str) -> bool {
    if direito.contains("internal_operations::aridade") || direito.contains("aridade_interna(") {
        return true;
    }
    direito.contains(".arity()")
        && (direito.contains("operation.")
            || direito.contains("entrada(")
            || direito.contains("internal_operations::"))
}

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

/// Nomes de constantes de `src/**` cujo VALOR é uma grafia interna.
///
/// Sem isto, renomear a constante — ou criar outra — apagaria a menção e a
/// grafia entraria por trás do guard. A resolução é por valor, não por
/// convenção de nome.
fn constantes_com_valor_de_grafia_interna(raiz: &Path) -> BTreeSet<String> {
    let mut fontes = Vec::new();
    fontes_rust(&raiz.join("src"), &mut fontes);
    let mut nomes = BTreeSet::new();
    for caminho in fontes {
        let texto = std::fs::read_to_string(&caminho).expect("ler fonte");
        for linha in texto.lines() {
            let linha = linha.trim();
            let Some(resto) = linha
                .strip_prefix("pub const ")
                .or(linha.strip_prefix("const "))
            else {
                continue;
            };
            if !(linha.contains("\"__pinker_internal_") || linha.contains("\"__ternario\"")) {
                continue;
            }
            let Some(nome) = resto.split(':').next() else {
                continue;
            };
            let nome = nome.trim();
            if !nome.is_empty() {
                nomes.insert(nome.to_string());
            }
        }
    }
    nomes
}

/// Posições de toda menção a uma operação interna: grafia literal, ou nome de
/// constante cujo valor é grafia interna.
fn mencoes_de_operacao_interna_com(texto: &str, constantes: &BTreeSet<String>) -> Vec<usize> {
    let mut posicoes = Vec::new();
    let mut marcadores: Vec<String> = vec![
        "\"__pinker_internal_".to_string(),
        "\"__ternario\"".to_string(),
    ];
    marcadores.extend(constantes.iter().cloned());
    for marcador in marcadores {
        let marcador = marcador.as_str();
        let mut base = 0usize;
        while let Some(deslocamento) = texto[base..].find(marcador) {
            posicoes.push(base + deslocamento);
            base += deslocamento + marcador.len();
        }
    }
    posicoes.sort_unstable();
    posicoes.dedup();
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
            if !e_consulta_a_autoridade(direito) {
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
            if relativo == Path::new(ORACULO_FILE) && grafia == GRAFIA_SINTETICA_DO_ORACULO {
                continue;
            }
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
fn guard_suplementar_nenhum_derivador_declara_contrato_ao_lado_da_grafia_interna() {
    let constantes = constantes_com_valor_de_grafia_interna(&repo());
    // Generaliza a forma física: não é só `insert(` numa tabela. Nomear a
    // operação e, na vizinhança, escrever tipo IR, tipo de pilha ou símbolo de
    // runtime é declarar contrato local — em `insert`, `match`, `if`, array ou
    // comparação.
    let raiz = repo();
    let mut ofensores = Vec::new();
    for (derivador, texto) in derivadores(&raiz) {
        for posicao in mencoes_de_operacao_interna_com(&texto, &constantes) {
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
            if !mencoes_de_operacao_interna_com(janela, &constantes).is_empty()
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
fn guard_suplementar_nenhum_derivador_responde_aridade_sem_perguntar_a_autoridade() {
    let constantes = constantes_com_valor_de_grafia_interna(&repo());
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
        for posicao in mencoes_de_operacao_interna_com(&texto, &constantes) {
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
fn a_autoridade_nao_declara_simbolo_abi() {
    // G651-01 — o binding `operação interna -> símbolo do runtime` tinha um
    // decisor só no baseline, `backend_s`, e a #651 só autoriza consolidar essa
    // relação quando várias fases repetem a MESMA relação. Que o backend também
    // enumere operações internas não torna o binding uma decisão duplicada:
    //
    // ```text
    // OPERATION EXISTS
    // !=
    // THIS OPERATION BINDS TO THIS ABI SYMBOL
    // ```
    //
    // O símbolo continua com o dono único preexistente, e esta autoridade não
    // pode readquiri-lo por conveniência de tabela.
    let fonte = std::fs::read_to_string(repo().join(AUTHORITY_FILE)).expect("ler autoridade");
    let corpo = fonte
        .split("pub const INTERNAL_OPERATIONS")
        .nth(1)
        .expect("tabela declarativa presente");
    assert!(
        !corpo.contains("\"pinker_"),
        "a autoridade declarativa voltou a declarar símbolo ABI"
    );
    assert!(
        !fonte.contains("fn simbolo_runtime"),
        "a autoridade voltou a responder pelo símbolo do runtime"
    );
}

#[test]
fn simbolo_abi_nunca_e_identidade_de_operacao_interna() {
    // O símbolo é projeção do backend. Nenhuma consulta da autoridade pode
    // aceitá-lo como chave, nem uma grafia interna pode coincidir com ele.
    for simbolo in [
        "pinker_mapa_definir",
        "pinker_leque_anexar",
        "pinker_leque_carga",
        "pinker_mapa_iterador_proxima",
    ] {
        assert!(
            !internal_operations::e_operacao_interna(simbolo),
            "o símbolo ABI '{simbolo}' foi aceito como identidade interna"
        );
        assert_eq!(internal_operations::aridade(simbolo), None);
        assert!(internal_operations::entrada(simbolo).is_none());
    }
    for operation in INTERNAL_OPERATIONS {
        assert!(
            !operation.spelling.starts_with("pinker_"),
            "identidade interna colapsou no namespace ABI: {}",
            operation.spelling
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

// ---------------------------------------------------------------------------
// 5. Guard estrutural SUPLEMENTAR sobre tokens
//
// ```text
// SUPPLEMENTAL_STRUCTURAL_GUARD
// NOT_SEMANTIC_AUTHORITY_PROOF
// ```
//
// As regras desta seção operam sobre o fluxo de tokens do Rust, com escopo de
// bloco balanceado, e são baratas de manter:
//
// ```text
// R-A  uma derivação não decide aridade         (nenhum literal, nenhuma fatia fixa)
// R-C  ninguém declara tabela local de grafias  (const/array de grafia interna)
// R-D  ninguém publica consulta de contrato     (assinatura devolve TypeIR por grafia)
// ```
//
// O que elas são: regression guard contra as formas JÁ VISTAS. Reintroduzir
// literalmente uma das reimplementações conhecidas volta a ficar vermelho aqui,
// barato e cedo.
//
// O que elas NÃO são: prova de que nenhuma implementação local pode existir.
// O fechamento dirigido do HEAD `f8683d87` falsificou essa leitura com 19
// bypasses materiais, e a antiga R-B — “uma derivação não busca contrato fora”
// — foi REMOVIDA por sobre-detecção: ela recusava um predicado de comparação de
// tipo legitimamente próprio do validador, que é fato de fase e não contrato
// compartilhado.
//
// A prova de autoridade única é a execução metamórfica em
// `src/internal_operations/metamorphic_oracle.rs`. Se alguma vez estas regras e
// aquele oráculo discordarem, o oráculo decide.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tipo {
    Ident,
    Texto,
    Numero,
    Pontuacao,
}

#[derive(Clone, Debug)]
struct Token {
    tipo: Tipo,
    texto: String,
    byte: usize,
}

/// Tokenizador do subconjunto de Rust que importa aqui.
///
/// Comentários somem, literais de texto viram o conteúdo, e os operadores de
/// dois caracteres viram um token só. Não é um parser: é o suficiente para
/// escopo de bloco e para distinguir literal de identificador — que é
/// exatamente o que as janelas de bytes não conseguiam fazer.
fn tokenizar(fonte: &str) -> Vec<Token> {
    let bytes = fonte.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if !c.is_ascii() {
            // Texto acentuado dentro de comentário ou identificador não-ASCII:
            // avança um CARACTERE, nunca um byte, para não fatiar no meio.
            i += 1;
            while i < bytes.len() && !fonte.is_char_boundary(i) {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if fonte[i..].starts_with("//") {
            i = fonte[i..].find('\n').map_or(bytes.len(), |d| i + d);
            continue;
        }
        if fonte[i..].starts_with("/*") {
            i = fonte[i..].find("*/").map_or(bytes.len(), |d| i + d + 2);
            continue;
        }
        // Literal cru: r"…", r#"…"#, r##"…"##
        if c == b'r' && fonte[i + 1..].starts_with(['"', '#']) {
            let mut cerquilhas = 0usize;
            let mut j = i + 1;
            while bytes.get(j) == Some(&b'#') {
                cerquilhas += 1;
                j += 1;
            }
            if bytes.get(j) == Some(&b'"') {
                let fechamento = format!("\"{}", "#".repeat(cerquilhas));
                let fim = fonte[j + 1..]
                    .find(&fechamento)
                    .map_or(bytes.len(), |d| j + 1 + d);
                tokens.push(Token {
                    tipo: Tipo::Texto,
                    texto: fonte[j + 1..fim].to_string(),
                    byte: i,
                });
                i = fim + fechamento.len();
                continue;
            }
        }
        if c == b'"' {
            let mut j = i + 1;
            while j < bytes.len() {
                match bytes[j] {
                    b'\\' => j += 2,
                    b'"' => break,
                    _ => j += 1,
                }
            }
            let fim = j.min(bytes.len());
            tokens.push(Token {
                tipo: Tipo::Texto,
                texto: fonte[i + 1..fim].to_string(),
                byte: i,
            });
            i = fim + 1;
            continue;
        }
        if c == b'\'' {
            // Tempo de vida ou literal de caractere: irrelevante para as regras.
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if bytes.get(j) == Some(&b'\'') {
                j += 1;
            }
            tokens.push(Token {
                tipo: Tipo::Pontuacao,
                texto: "'".to_string(),
                byte: i,
            });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            tokens.push(Token {
                tipo: Tipo::Numero,
                texto: fonte[i..j].to_string(),
                byte: i,
            });
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            tokens.push(Token {
                tipo: Tipo::Ident,
                texto: fonte[i..j].to_string(),
                byte: i,
            });
            i = j;
            continue;
        }
        let dois = fonte.get(i..i + 2).unwrap_or("");
        if matches!(
            dois,
            "==" | "!=" | "<=" | ">=" | "->" | "=>" | "::" | "&&" | "||"
        ) {
            tokens.push(Token {
                tipo: Tipo::Pontuacao,
                texto: dois.to_string(),
                byte: i,
            });
            i += 2;
            continue;
        }
        tokens.push(Token {
            tipo: Tipo::Pontuacao,
            texto: (c as char).to_string(),
            byte: i,
        });
        i += 1;
    }
    tokens
}

fn e_pontuacao(token: &Token, texto: &str) -> bool {
    token.tipo == Tipo::Pontuacao && token.texto == texto
}

/// Fim do bloco balanceado que começa em `abre`.
fn fim_do_bloco(tokens: &[Token], abre: usize) -> usize {
    let mut nivel = 0usize;
    for (indice, token) in tokens.iter().enumerate().skip(abre) {
        if e_pontuacao(token, "{") {
            nivel += 1;
        } else if e_pontuacao(token, "}") {
            nivel -= 1;
            if nivel == 0 {
                return indice;
            }
        }
    }
    tokens.len() - 1
}

/// Região de derivação aberta por um marcador da autoridade.
///
/// É o bloco que o próprio construto abre — o corpo do `if`, do `for` ou do
/// `else` de um `let … else`. Sem bloco no mesmo enunciado, a região vai do
/// marcador até o fim do bloco que o contém, que é o menor escopo honesto.
fn regiao_de_derivacao(tokens: &[Token], marcador: usize) -> (usize, usize) {
    // `let PAT = <consulta> else { … };` — o primeiro bloco é o ramo de ERRO, e
    // a derivação real vem DEPOIS do `;`. Tratar o ramo de erro como região
    // deixava toda a derivação fora do alcance, e a mesma decisão mudava de
    // veredito só por ter sido escrita alguns tokens adiante.
    if let Some(inicio_do_let) = comeco_do_let(tokens, marcador) {
        if let Some(ponto_e_virgula) = fim_do_let_com_else(tokens, inicio_do_let) {
            return (
                ponto_e_virgula,
                fim_do_bloco_que_contem(tokens, ponto_e_virgula),
            );
        }
    }
    let mut parenteses = 0i32;
    for indice in marcador..tokens.len() {
        let token = &tokens[indice];
        if e_pontuacao(token, "(") || e_pontuacao(token, "[") {
            parenteses += 1;
        } else if e_pontuacao(token, ")") || e_pontuacao(token, "]") {
            parenteses -= 1;
        } else if e_pontuacao(token, "{") {
            // O primeiro bloco depois do marcador é o corpo que ele abre:
            // ramo do `if`, corpo do `for`, corpo do fecho.
            return (indice, fim_do_bloco(tokens, indice));
        } else if e_pontuacao(token, "}") {
            // Saiu do bloco que contém o marcador sem abrir nenhum.
            return (marcador, indice);
        } else if e_pontuacao(token, ";") && parenteses <= 0 {
            return (marcador, indice);
        }
    }
    (marcador, tokens.len() - 1)
}

/// Início do `let` que contém `indice`, quando há um.
fn comeco_do_let(tokens: &[Token], indice: usize) -> Option<usize> {
    let piso = indice.saturating_sub(40);
    (piso..indice)
        .rev()
        .find(|i| tokens[*i].tipo == Tipo::Ident && tokens[*i].texto == "let")
}

/// Posição do `;` do `let`, mas só quando ele tem um ramo `else`.
fn fim_do_let_com_else(tokens: &[Token], inicio: usize) -> Option<usize> {
    let mut tem_else = false;
    let mut chaves = 0i32;
    let mut parenteses = 0i32;
    for (indice, token) in tokens.iter().enumerate().skip(inicio) {
        if e_pontuacao(token, "(") || e_pontuacao(token, "[") {
            parenteses += 1;
        } else if e_pontuacao(token, ")") || e_pontuacao(token, "]") {
            parenteses -= 1;
        } else if e_pontuacao(token, "{") {
            chaves += 1;
        } else if e_pontuacao(token, "}") {
            chaves -= 1;
            if chaves < 0 {
                return None;
            }
        } else if token.tipo == Tipo::Ident && token.texto == "else" && chaves == 0 {
            tem_else = true;
        } else if e_pontuacao(token, ";") && chaves == 0 && parenteses <= 0 {
            return tem_else.then_some(indice);
        }
    }
    None
}

/// Fim do bloco que contém `indice`, sem exigir que ele o abra.
fn fim_do_bloco_que_contem(tokens: &[Token], indice: usize) -> usize {
    let mut nivel = 0i32;
    for (atual, token) in tokens.iter().enumerate().skip(indice) {
        if e_pontuacao(token, "{") {
            nivel += 1;
        } else if e_pontuacao(token, "}") {
            if nivel == 0 {
                return atual;
            }
            nivel -= 1;
        }
    }
    tokens.len() - 1
}

/// O identificador nomeia a autoridade das operações internas?
fn e_da_autoridade(tokens: &[Token], indice: usize) -> bool {
    // Toda função que já deriva da autoridade abre região: `is_generic_map_intrinsic`
    // é derivação de `ir::model`, e o ramo que ela guarda é derivação tanto
    // quanto o que consulta `entrada` diretamente.
    matches!(
        tokens[indice].texto.as_str(),
        "internal_operations" | "aridade_interna" | "is_generic_map_intrinsic"
    ) || (tokens[indice].texto == "arity"
        && indice >= 2
        && matches!(
            tokens[indice - 2].texto.as_str(),
            "operation" | "entrada" | "internal_operations"
        ))
}

/// Padrão de fatia de comprimento fixo começando em `indice`.
///
/// Conta ELEMENTOS, não a forma deles: `[_, _, _]` e
/// `[cond, entao, senao]` fixam a mesma aridade, e destruturar com nomes é a
/// forma idiomática — foi por ela que a versão anterior passou.
/// `[a, b, ..]` não fixa comprimento e não conta.
fn fatia_de_comprimento_fixo(tokens: &[Token], indice: usize) -> Option<usize> {
    if !e_pontuacao(&tokens[indice], "[") {
        return None;
    }
    // Só é PADRÃO depois de `let`, de `=>` ou dentro de `matches!`.
    let anterior = &tokens[indice.checked_sub(1)?];
    let contexto_de_padrao = anterior.texto == "let"
        || anterior.texto == "=>"
        || anterior.texto == ","
        || e_pontuacao(anterior, "(");
    if !contexto_de_padrao {
        return None;
    }
    let mut elementos = 1usize;
    let mut profundidade = 0i32;
    for token in tokens.iter().skip(indice + 1) {
        if e_pontuacao(token, "[") || e_pontuacao(token, "(") || e_pontuacao(token, "{") {
            profundidade += 1;
        } else if e_pontuacao(token, ")") || e_pontuacao(token, "}") {
            profundidade -= 1;
        } else if e_pontuacao(token, "]") {
            if profundidade == 0 {
                return (elementos > 1).then_some(elementos);
            }
            profundidade -= 1;
        } else if e_pontuacao(token, "..") && profundidade == 0 {
            return None;
        } else if e_pontuacao(token, ",") && profundidade == 0 {
            elementos += 1;
        } else if e_pontuacao(token, ";") {
            return None;
        }
    }
    None
}

/// R-A — uma derivação não decide aridade.
///
/// Qualquer literal numérico em comparação, e qualquer padrão de fatia de
/// comprimento fixo, é resposta que não veio da autoridade. Não importa se o
/// número está à direita de `!=`, de `<`, de `matches!` ou de um `let [a, b, c]`.
fn decide_aridade(tokens: &[Token], inicio: usize, fim: usize) -> Option<String> {
    for indice in inicio..fim {
        let token = &tokens[indice];
        if token.tipo == Tipo::Numero
            && indice > 0
            && matches!(
                tokens[indice - 1].texto.as_str(),
                "==" | "!=" | "<" | ">" | "<=" | ">="
            )
        {
            return Some(format!("literal '{}' em comparação", token.texto));
        }
        if let Some(elementos) = fatia_de_comprimento_fixo(tokens, indice) {
            return Some(format!("padrão de fatia de {elementos} elementos"));
        }
    }
    None
}

/// R-D — ninguém publica consulta de contrato chaveada por grafia.
///
/// A fachada não se reconhece por onde mora nem por como é chamada: reconhece-se
/// pela ASSINATURA. Uma função cujo tipo de retorno menciona a representação de
/// contrato e cujo corpo nomeia uma grafia interna é tabela de contrato, esteja
/// ela num validador, num produtor ou num arquivo novo, e seja ela chamada
/// diretamente ou por ponteiro ligado a um local.
///
/// Produtor legítimo devolve nó de IR, enunciado ou expressão — nunca `TypeIR`
/// como resposta sobre uma grafia.
fn recebe_grafia(tokens: &[Token], inicio: usize, fim: usize) -> bool {
    let Some(abre) = (inicio..fim).find(|i| e_pontuacao(&tokens[*i], "(")) else {
        return false;
    };
    let mut profundidade = 0i32;
    let mut tipo: Vec<&str> = Vec::new();
    let mut depois_dos_dois_pontos = false;
    for token in tokens.iter().take(fim).skip(abre + 1) {
        if e_pontuacao(token, "(") || e_pontuacao(token, "<") || e_pontuacao(token, "[") {
            profundidade += 1;
        } else if e_pontuacao(token, ">") || e_pontuacao(token, "]") {
            profundidade -= 1;
        } else if e_pontuacao(token, ")") {
            if profundidade == 0 {
                break;
            }
            profundidade -= 1;
        } else if e_pontuacao(token, ",") && profundidade == 0 {
            if tipo == ["&", "str"] || tipo == ["&", "String"] {
                return true;
            }
            tipo.clear();
            depois_dos_dois_pontos = false;
        } else if e_pontuacao(token, ":") && profundidade == 0 {
            depois_dos_dois_pontos = true;
        } else if depois_dos_dois_pontos && token.texto != "'" {
            tipo.push(&token.texto);
        }
    }
    tipo == ["&", "str"] || tipo == ["&", "String"]
}

fn consultas_de_contrato_por_grafia(tokens: &[Token]) -> Vec<String> {
    let mut achados = Vec::new();
    for (indice, token) in tokens.iter().enumerate() {
        if token.tipo != Tipo::Ident || token.texto != "fn" {
            continue;
        }
        let Some(nome) = tokens.get(indice + 1) else {
            continue;
        };
        // Assinatura: do `fn` até o `{` que abre o corpo.
        let mut abre = indice;
        while abre < tokens.len() && !e_pontuacao(&tokens[abre], "{") {
            if e_pontuacao(&tokens[abre], ";") {
                break;
            }
            abre += 1;
        }
        if abre >= tokens.len() || !e_pontuacao(&tokens[abre], "{") {
            continue;
        }
        let assinatura_devolve_contrato = tokens[indice..abre]
            .iter()
            .any(|t| matches!(t.texto.as_str(), "TypeIR" | "StackValueType"));
        // Chaveada POR GRAFIA: algum parâmetro é exatamente `&str`/`&String`.
        // Um validador que infere tipo a partir de um nó de IR também devolve
        // `TypeIR` e carrega `HashMap<String, TypeIR>` — a diferença está na
        // CHAVE, não no retorno, e `String` dentro de container não é chave.
        let chaveada_por_grafia = recebe_grafia(tokens, indice, abre);
        if !assinatura_devolve_contrato || !chaveada_por_grafia {
            continue;
        }
        let fim = fim_do_bloco(tokens, abre);
        let nomeia_grafia = tokens[abre..fim]
            .iter()
            .any(|t| t.tipo == Tipo::Texto && e_grafia_interna(&t.texto));
        if nomeia_grafia {
            achados.push(format!(
                "'{}' devolve contrato a partir de grafia interna",
                nome.texto
            ));
        }
    }
    achados
}

/// R-C — ninguém declara tabela local de grafias internas.
///
/// A exceção é a forma que a própria autoridade consome: `const NOME: &str =
/// "<grafia>"`, e somente quando a autoridade referencia `NOME`. Array de
/// grafias, `static`, ou constante que a autoridade não conhece são tabela
/// local, e não passam.
fn tabelas_locais_de_grafia(tokens: &[Token], fonte_da_autoridade: &str) -> Vec<String> {
    let mut achados = Vec::new();
    for (indice, token) in tokens.iter().enumerate() {
        if token.tipo != Tipo::Ident || !matches!(token.texto.as_str(), "const" | "static") {
            continue;
        }
        let Some(nome) = tokens.get(indice + 1) else {
            continue;
        };
        let mut fim = indice;
        while fim < tokens.len() && !e_pontuacao(&tokens[fim], ";") {
            fim += 1;
        }
        let grafias: Vec<&Token> = tokens[indice..fim]
            .iter()
            .filter(|t| t.tipo == Tipo::Texto && e_grafia_interna(&t.texto))
            .collect();
        if grafias.is_empty() {
            continue;
        }
        let declaracao_de_grafia_unica = grafias.len() == 1
            && tokens[indice..fim].iter().any(|t| t.texto == "str")
            && !tokens[indice..fim].iter().any(|t| e_pontuacao(t, "["));
        if declaracao_de_grafia_unica && fonte_da_autoridade.contains(&nome.texto) {
            continue;
        }
        achados.push(format!(
            "'{}' declara {} grafia(s) interna(s) fora da autoridade",
            nome.texto,
            grafias.len()
        ));
    }
    achados
}

fn e_grafia_interna(valor: &str) -> bool {
    (valor.starts_with("__pinker_internal_") && valor.len() > "__pinker_internal_".len())
        || valor == "__ternario"
}

#[test]
fn guard_suplementar_nenhuma_derivacao_decide_por_conta_propria() {
    let raiz = repo();
    let autoridade = std::fs::read_to_string(raiz.join(AUTHORITY_FILE)).expect("ler autoridade");
    let mut fontes = Vec::new();
    fontes_rust(&raiz.join("src"), &mut fontes);

    let mut ofensores = Vec::new();
    for caminho in fontes {
        let relativo = caminho
            .strip_prefix(&raiz)
            .expect("caminho relativo")
            .to_string_lossy()
            .into_owned();
        let fonte = std::fs::read_to_string(&caminho).expect("ler fonte");
        let tokens = tokenizar(&fonte);

        // R-C vale para a árvore inteira menos a autoridade: uma tabela de
        // grafias é tabela onde quer que ela more.
        // `native_symbol` é a autoridade de identidade: a tabela de namespaces
        // reservados PRECISA nomear `__ternario`, e é dela que
        // `is_compiler_generated` deriva. Não é tabela de contrato.
        if relativo != AUTHORITY_FILE
            && relativo != ORACLE_FILE
            && relativo != "src/native_symbol.rs"
        {
            for achado in tabelas_locais_de_grafia(&tokens, &autoridade) {
                ofensores.push(format!("{relativo}: R-C {achado}"));
            }
            for achado in consultas_de_contrato_por_grafia(&tokens) {
                ofensores.push(format!("{relativo}: R-D {achado}"));
            }
        }

        // R-A e R-B valem nas regiões abertas por um marcador da autoridade:
        // é lá que a fase já recebeu a resposta e não tem o que decidir.
        for (indice, token) in tokens.iter().enumerate() {
            if token.tipo != Tipo::Ident || !e_da_autoridade(&tokens, indice) {
                continue;
            }
            if !tokens
                .get(indice + 1)
                .is_some_and(|t| e_pontuacao(t, "::") || e_pontuacao(t, "(") || e_pontuacao(t, "."))
            {
                continue;
            }
            let (inicio, fim) = regiao_de_derivacao(&tokens, indice);
            let linha = fonte[..token.byte].matches('\n').count() + 1;
            if let Some(motivo) = decide_aridade(&tokens, inicio, fim) {
                ofensores.push(format!("{relativo}:{linha} R-A {motivo}"));
            }
        }
    }
    ofensores.sort();
    ofensores.dedup();
    assert!(
        ofensores.is_empty(),
        "decisão local sobre operação interna reintroduzida: {ofensores:#?}"
    );
}
