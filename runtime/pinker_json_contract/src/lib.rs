//! Contrato puro do valor JSON da Pinker — Parte E1.
//!
//! Este crate existe por uma razão única e verificável: **uma** implementação.
//!
//! O interpretador vive no crate do compilador e o runtime nativo vive em
//! `pinker_rt`, que não pode depender do compilador. Duas implementações da
//! mesma gramática divergiriam — foi exatamente assim que a família JSON
//! anterior acabou funcionando no interpretador e inexistindo no nativo.
//!
//! ```text
//! ONE_GRAMMAR -> ONE_IMPLEMENTATION -> PARITY_BY_CONSTRUCTION
//! ```
//!
//! Aqui não há nome público da linguagem, tipo do compilador nem ABI: só o
//! modelo, a interpretação de texto externo e a serialização determinística.
//! Os nomes públicos e as assinaturas vivem em `valor_json`, no compilador.
//!
//! Mesma disciplina do `pinker_memory_contract`: contrato puro, compartilhado
//! pelas duas pontas, sem dependência externa.

use std::collections::BTreeMap;

// @pinker-nav:start json.valor.modelo
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única do valor JSON da Parte E1: `NoJson` é o nó da arena (nulo, lógica, número `i64`, verso, lista de handles e objeto `BTreeMap` de handles), `TabelaJson` materializa a árvore atrás de handles monotônicos que nunca são reutilizados, e `TipoJson` nomeia as seis classes observáveis na ordem de declaração que é o discriminante lido pela IR. O nesting é recursivo por handle, não por família de formato: nenhum helper novo é exigido por forma nova. Objeto usa `BTreeMap`, então a ordem observável é das chaves por construção e nunca herdada de `HashMap`.

/// Variantes do leque `TipoJson`, em ordem de declaração.
///
/// A ordem **é** o discriminante lido pela IR, como nas variantes de
/// `TipoEntrada`. O nome público do leque vive no compilador; a tabela vive
/// aqui para que o runtime nativo espelhe os mesmos discriminantes sem
/// redeclará-los. Reordenar quebra a suíte em vez de corromper valores em
/// silêncio.
pub const VARIANTES: [&str; 6] = ["Objeto", "Lista", "Verso", "Numero", "Logica", "Nulo"];

/// Classe observável de um valor JSON.
///
/// `Nulo` está aqui por decisão explícita, não por analogia com a lista da
/// campanha: JSON real contém `null`, e recusá-lo tornaria documentos legítimos
/// inparseáveis. Ele não vira valor Pinker — `nulo` é ausência de retorno no
/// sistema de tipos, não um valor de primeira classe. Vira **tag**, observável
/// apenas por `json_tipo`, sem acessor próprio. Custo de representação
/// nova: zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoJson {
    Objeto,
    Lista,
    Verso,
    Numero,
    Logica,
    Nulo,
}

impl TipoJson {
    /// Discriminante lido pela IR: o índice de declaração em [`VARIANTES`].
    pub fn discriminante(self) -> u64 {
        match self {
            TipoJson::Objeto => 0,
            TipoJson::Lista => 1,
            TipoJson::Verso => 2,
            TipoJson::Numero => 3,
            TipoJson::Logica => 4,
            TipoJson::Nulo => 5,
        }
    }

    /// Nome público da variante.
    pub fn nome(self) -> &'static str {
        VARIANTES[self.discriminante() as usize]
    }
}

/// Nó da arena.
///
/// Os filhos são handles da **mesma** tabela. É isso que torna o nesting geral:
/// a profundidade não aparece no tipo do nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoJson {
    Nulo,
    Logica(bool),
    /// Domínio numérico suportado: `i64` exato.
    ///
    /// Não existe conversão lossy. Fração, expoente e magnitude fora de `i64`
    /// são recusados na origem, como falha recuperável — nunca aproximados.
    Numero(i64),
    Verso(String),
    Lista(Vec<u64>),
    /// `BTreeMap` e não `HashMap`: a ordem observável das chaves é uma
    /// propriedade da estrutura, não um acidente de iteração.
    Objeto(BTreeMap<String, u64>),
}

impl NoJson {
    /// Classe observável do nó.
    pub fn tipo(&self) -> TipoJson {
        match self {
            NoJson::Nulo => TipoJson::Nulo,
            NoJson::Logica(_) => TipoJson::Logica,
            NoJson::Numero(_) => TipoJson::Numero,
            NoJson::Verso(_) => TipoJson::Verso,
            NoJson::Lista(_) => TipoJson::Lista,
            NoJson::Objeto(_) => TipoJson::Objeto,
        }
    }
}

/// Política de lifetime da árvore, declarada explicitamente.
///
/// Existe como item nomeado, e não como comentário, porque a D13 exige que uma
/// família de valor por handle responda essas perguntas **antes** de ser
/// implementada.
pub struct PoliticaValorJson;

impl PoliticaValorJson {
    /// A árvore é retida até o fim do programa.
    ///
    /// Custo real e assumido, idêntico ao de listas, mapas, callables e
    /// snapshots de processo: um documento grande permanece em memória. Não é
    /// exceção aberta para JSON.
    pub const RETIDO_ATE_O_FIM: bool = true;

    /// Handles são monotônicos e nunca reutilizados.
    ///
    /// `stale alias` e ABA são impossíveis por construção, não por disciplina.
    pub const HANDLE_REUTILIZADO: bool = false;

    /// Nós são imutáveis depois de criados.
    ///
    /// Consequência: uma cópia do handle observa o mesmo nó, e nenhuma mutação
    /// pode ser observada através de um alias.
    pub const MUTAVEL_APOS_CRIACAO: bool = false;

    /// Nenhum recurso de sistema operacional vive atrás do handle.
    pub const RECURSO_DE_SO_VIVO: bool = false;

    /// A arena é recursiva: um nó referencia handles da mesma tabela.
    ///
    /// Este é o **único** delta material da família em relação à política já
    /// vigente, e está registrado para o `D13_RECONCILIATION_GATE`. Ele não
    /// muda release, reuso nem keepalive: como nada é removido, um filho nunca
    /// pode ser liberado antes do pai.
    pub const ARENA_RECURSIVA: bool = true;
}

/// Profundidade máxima aceita ao interpretar texto externo.
///
/// Contenção, não gosto: o parser é recursivo, e texto externo hostil como
/// `[[[[...]]]]` levaria a pilha do host abaixo. O limite transforma isso em
/// falha recuperável.
///
/// ```text
/// SUBJECT_MAY_FAIL_BUT_HOST_MUST_SURVIVE
/// ```
pub const LIMITE_PROFUNDIDADE: usize = 128;

/// Tabela de nós do runtime.
///
/// Mesma forma de `TabelaSaidas`/`RuntimeListState`/`CallableState`: mapa por
/// handle mais contador monotônico, sem caminho de remoção.
#[derive(Debug)]
pub struct TabelaJson {
    entradas: std::collections::HashMap<u64, NoJson>,
    proximo_handle: Option<u64>,
}

/// `Default` **não** pode ser derivado: `Option::default()` é `None`, que aqui
/// significa "namespace de handles esgotado". Uma tabela recém-criada por
/// `default()` recusaria o primeiro nó — e recusaria apenas em quem usa
/// `default()`, que foi como o runtime nativo divergiu do interpretador antes
/// desta implementação existir.
impl Default for TabelaJson {
    fn default() -> Self {
        Self::nova()
    }
}

impl TabelaJson {
    pub fn nova() -> Self {
        Self {
            entradas: std::collections::HashMap::new(),
            proximo_handle: Some(1),
        }
    }

    /// Materializa um nó e devolve seu handle.
    pub fn inserir(&mut self, no: NoJson) -> u64 {
        let handle = self
            .proximo_handle
            .expect("invariante interna violada: namespace de handles de ValorJson esgotado");
        assert!(
            !self.entradas.contains_key(&handle),
            "invariante interna violada: handle de ValorJson seria reutilizado"
        );
        let proximo = handle.checked_add(1);
        self.entradas.insert(handle, no);
        self.proximo_handle = proximo;
        handle
    }

    /// Lê um nó já materializado.
    ///
    /// `None` significa handle não produzido por esta tabela — violação de
    /// invariante interna, não falha operacional do programa do usuário.
    pub fn obter(&self, handle: u64) -> Option<&NoJson> {
        self.entradas.get(&handle)
    }

    /// Quantidade de nós retidos. Serve à evidência da política de lifetime.
    pub fn retidos(&self) -> usize {
        self.entradas.len()
    }
}
// @pinker-nav:end json.valor.modelo

// @pinker-nav:start json.texto.interpretacao
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Interpretação de texto JSON externo em árvore de arena: descida recursiva com limite de profundidade explícito, números restritos ao domínio `i64` exato sem fração nem expoente, strings com escapes completos e pares surrogate validados, chave duplicada recusada e lixo após o valor recusado. Toda recusa é falha recuperável descrita em texto — nunca pânico, nunca aproximação silenciosa, nunca comportamento herdado de biblioteca host.
/// Inteiro JSON validado, ainda **sem** domínio escolhido.
///
/// Sinal e magnitude separados: é a menor forma que representa exatamente tudo
/// que as duas projeções precisam, sem que nenhuma delas minta sobre a outra.
/// `-0` é aceito pela gramática e projeta para `0` nas duas superfícies, porque
/// zero não tem sinal no domínio de destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteiroExato {
    negativo: bool,
    magnitude: u64,
}

impl InteiroExato {
    /// Projeção do modelo adulto: `i64` exato.
    ///
    /// `i64::MIN` só é alcançável pela magnitude `2^63`, que não cabe em `i64`
    /// positivo — por isso a negação é feita em `i128` e só então estreitada.
    pub fn como_i64(self) -> Result<i64, String> {
        const MAGNITUDE_MINIMA: u64 = 1 << 63; // |i64::MIN|
        if self.negativo {
            if self.magnitude > MAGNITUDE_MINIMA {
                return Err("json inválido: número fora do domínio suportado".to_string());
            }
            Ok(-(i128::from(self.magnitude)) as i64)
        } else {
            i64::try_from(self.magnitude)
                .map_err(|_| "json inválido: número fora do domínio suportado".to_string())
        }
    }

    /// Projeção do recorte plano histórico: `u64` sem sinal.
    ///
    /// Preserva a faixa inteira do recorte anterior, inclusive
    /// `i64::MAX + 1 ..= u64::MAX`, que o domínio adulto recusa. As duas
    /// respostas são corretas para superfícies diferentes.
    pub fn como_u64(self) -> Option<u64> {
        // Negativo nunca pertenceu ao recorte plano. `-0` é zero.
        if self.negativo && self.magnitude != 0 {
            return None;
        }
        Some(self.magnitude)
    }
}

/// Interpreta texto JSON externo, materializando a árvore em `tabela`.
///
/// Devolve o handle da raiz, ou a causa em texto. Toda falha aqui é **dado
/// externo malformado**: recuperável por contrato, distinta de erro estático de
/// programa e de violação de invariante interna.
pub fn interpretar(texto: &str, tabela: &mut TabelaJson) -> Result<u64, String> {
    let mut cursor = Cursor::new(texto);
    cursor.pular_espaco();
    let raiz = cursor.valor(tabela, 0)?;
    cursor.pular_espaco();
    if !cursor.fim() {
        return Err("json inválido: conteúdo extra após o valor".to_string());
    }
    Ok(raiz)
}

struct Cursor<'a> {
    src: &'a str,
    idx: usize,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, idx: 0 }
    }

    fn fim(&self) -> bool {
        self.idx >= self.src.len()
    }

    fn espiar(&self) -> Option<char> {
        self.src[self.idx..].chars().next()
    }

    fn pular_espaco(&mut self) {
        while let Some(ch) = self.espiar() {
            // Espaço em branco de JSON é exatamente este conjunto.
            if matches!(ch, ' ' | '\t' | '\n' | '\r') {
                self.idx += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consumir(&mut self, esperado: char) -> bool {
        if self.espiar() == Some(esperado) {
            self.idx += esperado.len_utf8();
            true
        } else {
            false
        }
    }

    fn exigir(&mut self, esperado: char) -> Result<(), String> {
        if self.consumir(esperado) {
            Ok(())
        } else {
            Err(format!("json inválido: esperado '{}'", esperado))
        }
    }

    fn consumir_literal(&mut self, literal: &str) -> bool {
        if self.src[self.idx..].starts_with(literal) {
            self.idx += literal.len();
            true
        } else {
            false
        }
    }

    /// Um valor JSON qualquer. `profundidade` cresce a cada nível de agregado.
    fn valor(&mut self, tabela: &mut TabelaJson, profundidade: usize) -> Result<u64, String> {
        if profundidade > LIMITE_PROFUNDIDADE {
            return Err(format!(
                "json inválido: profundidade acima do limite de {}",
                LIMITE_PROFUNDIDADE
            ));
        }
        match self.espiar() {
            None => Err("json inválido: valor ausente".to_string()),
            Some('{') => self.objeto(tabela, profundidade),
            Some('[') => self.lista(tabela, profundidade),
            Some('"') => {
                let texto = self.string()?;
                Ok(tabela.inserir(NoJson::Verso(texto)))
            }
            Some('t') => {
                if self.consumir_literal("true") {
                    Ok(tabela.inserir(NoJson::Logica(true)))
                } else {
                    Err("json inválido: literal desconhecido".to_string())
                }
            }
            Some('f') => {
                if self.consumir_literal("false") {
                    Ok(tabela.inserir(NoJson::Logica(false)))
                } else {
                    Err("json inválido: literal desconhecido".to_string())
                }
            }
            Some('n') => {
                if self.consumir_literal("null") {
                    Ok(tabela.inserir(NoJson::Nulo))
                } else {
                    Err("json inválido: literal desconhecido".to_string())
                }
            }
            Some(ch) if ch == '-' || ch.is_ascii_digit() => {
                // Projeção adulta: `i64` exato. Fora da faixa é falha
                // recuperável, nunca aproximação.
                let numero = self.numero()?.como_i64()?;
                Ok(tabela.inserir(NoJson::Numero(numero)))
            }
            Some(_) => Err("json inválido: valor desconhecido".to_string()),
        }
    }

    fn objeto(&mut self, tabela: &mut TabelaJson, profundidade: usize) -> Result<u64, String> {
        self.exigir('{')?;
        let mut membros: BTreeMap<String, u64> = BTreeMap::new();
        self.pular_espaco();
        if self.consumir('}') {
            return Ok(tabela.inserir(NoJson::Objeto(membros)));
        }
        loop {
            self.pular_espaco();
            let chave = self.string()?;
            if membros.contains_key(&chave) {
                // Política explícita: recusar. Não "first wins" nem "last
                // wins", que descartariam dado silenciosamente.
                return Err("json inválido: chave duplicada no objeto".to_string());
            }
            self.pular_espaco();
            self.exigir(':')?;
            self.pular_espaco();
            let valor = self.valor(tabela, profundidade + 1)?;
            membros.insert(chave, valor);
            self.pular_espaco();
            if self.consumir('}') {
                return Ok(tabela.inserir(NoJson::Objeto(membros)));
            }
            self.exigir(',')?;
        }
    }

    fn lista(&mut self, tabela: &mut TabelaJson, profundidade: usize) -> Result<u64, String> {
        self.exigir('[')?;
        let mut itens: Vec<u64> = Vec::new();
        self.pular_espaco();
        if self.consumir(']') {
            return Ok(tabela.inserir(NoJson::Lista(itens)));
        }
        loop {
            self.pular_espaco();
            let valor = self.valor(tabela, profundidade + 1)?;
            itens.push(valor);
            self.pular_espaco();
            if self.consumir(']') {
                return Ok(tabela.inserir(NoJson::Lista(itens)));
            }
            self.exigir(',')?;
        }
    }

    /// String JSON com escapes completos.
    ///
    /// Nada aqui é herdado de biblioteca host: cada recusa é uma decisão desta
    /// autoridade, e cada uma tem teste.
    fn string(&mut self) -> Result<String, String> {
        self.exigir('"')?;
        let mut saida = String::new();
        loop {
            let Some(ch) = self.espiar() else {
                return Err("json inválido: string não terminada".to_string());
            };
            match ch {
                '"' => {
                    self.idx += 1;
                    return Ok(saida);
                }
                '\\' => {
                    self.idx += 1;
                    let Some(escape) = self.espiar() else {
                        return Err("json inválido: escape não terminado".to_string());
                    };
                    self.idx += escape.len_utf8();
                    match escape {
                        '"' => saida.push('"'),
                        '\\' => saida.push('\\'),
                        '/' => saida.push('/'),
                        'b' => saida.push('\u{0008}'),
                        'f' => saida.push('\u{000C}'),
                        'n' => saida.push('\n'),
                        'r' => saida.push('\r'),
                        't' => saida.push('\t'),
                        'u' => saida.push(self.escape_unicode()?),
                        _ => return Err("json inválido: escape desconhecido".to_string()),
                    }
                }
                // Controle não escapado é inválido em JSON. A recusa é
                // explícita para não depender do host.
                _ if (ch as u32) < 0x20 => {
                    return Err("json inválido: caractere de controle não escapado".to_string());
                }
                _ => {
                    saida.push(ch);
                    self.idx += ch.len_utf8();
                }
            }
        }
    }

    /// Quatro dígitos hexadecimais depois de `\u`, com par surrogate quando
    /// o primeiro valor for um high surrogate.
    fn escape_unicode(&mut self) -> Result<char, String> {
        let primeiro = self.hex4()?;
        // Fora da faixa de surrogates: codepoint direto.
        if !(0xD800..=0xDFFF).contains(&primeiro) {
            return char::from_u32(primeiro)
                .ok_or_else(|| "json inválido: escape unicode fora do domínio".to_string());
        }
        // Low surrogate sozinho nunca é válido.
        if (0xDC00..=0xDFFF).contains(&primeiro) {
            return Err("json inválido: surrogate baixo isolado".to_string());
        }
        // High surrogate exige o par completo.
        if !self.consumir('\\') || !self.consumir('u') {
            return Err("json inválido: surrogate alto sem par".to_string());
        }
        let segundo = self.hex4()?;
        if !(0xDC00..=0xDFFF).contains(&segundo) {
            return Err("json inválido: par surrogate inválido".to_string());
        }
        let combinado = 0x10000 + ((primeiro - 0xD800) << 10) + (segundo - 0xDC00);
        char::from_u32(combinado)
            .ok_or_else(|| "json inválido: par surrogate fora do domínio".to_string())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let resto = &self.src[self.idx..];
        if resto.len() < 4 || !resto.is_char_boundary(4) {
            return Err("json inválido: escape unicode incompleto".to_string());
        }
        let digitos = &resto[..4];
        if !digitos.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err("json inválido: escape unicode não hexadecimal".to_string());
        }
        self.idx += 4;
        u32::from_str_radix(digitos, 16)
            .map_err(|_| "json inválido: escape unicode não hexadecimal".to_string())
    }

    /// Número JSON restrito ao domínio exatamente representável.
    ///
    /// Aceita apenas inteiros de `i64`. Fração e expoente são **recusados**, não
    /// aproximados: a Pinker não sustenta float, e converter perderia dado.
    /// Número JSON como inteiro **exato**, sem escolher domínio.
    ///
    /// O lexer não sabe — e não pode saber — qual superfície vai consumir o
    /// número. Decidir aqui por `i64` faria o recorte plano histórico, cujo
    /// domínio é `u64`, perder `i64::MAX + 1 ..= u64::MAX`. Decidir por `u64`
    /// tornaria negativo inexprimível para o modelo adulto.
    ///
    /// ```text
    /// SAME_JSON_LEXICAL_GRAMMAR
    /// DOES_NOT_IMPLY
    /// SAME_PROJECTION_DOMAIN
    /// ```
    ///
    /// A magnitude é acumulada com aritmética checada: estouro é detectado
    /// **antes** de truncar ou dar a volta.
    fn numero(&mut self) -> Result<InteiroExato, String> {
        let negativo = self.consumir('-');
        let mut magnitude: u64 = 0;
        let mut algum = false;
        // Regra de zero à esquerda do próprio JSON.
        if self.consumir('0') {
            if matches!(self.espiar(), Some(ch) if ch.is_ascii_digit()) {
                return Err("json inválido: zero à esquerda em número".to_string());
            }
            algum = true;
        } else {
            while let Some(ch) = self.espiar() {
                let Some(digito) = ch.to_digit(10) else { break };
                self.idx += 1;
                algum = true;
                magnitude = magnitude
                    .checked_mul(10)
                    .and_then(|acumulado| acumulado.checked_add(u64::from(digito)))
                    .ok_or_else(|| {
                        "json inválido: número acima da magnitude representável".to_string()
                    })?;
            }
        }
        if !algum {
            return Err("json inválido: número sem dígitos".to_string());
        }
        if matches!(self.espiar(), Some('.')) {
            return Err("json inválido: número com fração fora do domínio suportado".to_string());
        }
        if matches!(self.espiar(), Some('e') | Some('E')) {
            return Err("json inválido: número com expoente fora do domínio suportado".to_string());
        }
        Ok(InteiroExato {
            negativo,
            magnitude,
        })
    }
}
// @pinker-nav:end json.texto.interpretacao

// @pinker-nav:start json.plano.projecao-legada
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Projeção plana histórica sobre a MESMA autoridade léxica e sintática: `interpretar_plano_bombom` percorre o objeto de um nível com o cursor compartilhado e projeta cada número para `u64`, preservando a faixa inteira do recorte anterior — inclusive `i64::MAX + 1 ..= u64::MAX`, que o domínio adulto recusa —, e `serializar_plano_bombom` emite decimal exato sem cast para `i64`, com chaves ordenadas. As recusas históricas continuam recusando com as mesmas razões observáveis; o que mudou é que não existe mais um segundo cursor capaz de divergir.

/// Interpreta um objeto JSON plano no recorte histórico `verso -> bombom`.
///
/// Compartilha gramática com [`interpretar`] e diverge **apenas** na projeção
/// numérica: aqui o domínio é `u64`, lá é `i64`. Nenhum valor atravessa
/// `NoJson::Numero`, que mentiria sobre o próprio domínio ao receber
/// `u64::MAX`.
///
/// Devolve os pares já em ordem de chave.
pub fn interpretar_plano_bombom(texto: &str) -> Result<Vec<(String, u64)>, String> {
    let mut cursor = Cursor::new(texto);
    cursor.pular_espaco();
    if cursor.espiar() != Some('{') {
        // Mensagem histórica preservada: lista no topo continua recusada assim.
        return Err("esperado '{'".to_string());
    }
    cursor.idx += 1;
    let mut pares: BTreeMap<String, u64> = BTreeMap::new();
    cursor.pular_espaco();
    if cursor.consumir('}') {
        cursor.pular_espaco();
        return fim_do_plano(&cursor).map(|()| pares.into_iter().collect());
    }
    loop {
        cursor.pular_espaco();
        if cursor.espiar() != Some('"') {
            return Err("string de chave não terminada".to_string());
        }
        let chave = cursor.string().map_err(traduzir_erro_de_chave)?;
        validar_chave_plana(&chave)?;
        if pares.contains_key(&chave) {
            return Err("chave duplicada fora do recorte auditável".to_string());
        }
        cursor.pular_espaco();
        if !cursor.consumir(':') {
            return Err("esperado ':'".to_string());
        }
        cursor.pular_espaco();
        // Qualquer valor que não seja número — objeto, lista, string, booleano,
        // null — recusa com a razão histórica: o recorte plano só tem `bombom`.
        let valor = match cursor.espiar() {
            Some(ch) if ch == '-' || ch.is_ascii_digit() => cursor.numero()?,
            _ => return Err("valor deve ser bombom sem sinal".to_string()),
        };
        let valor = valor
            .como_u64()
            .ok_or_else(|| "valor deve ser bombom sem sinal".to_string())?;
        pares.insert(chave, valor);
        cursor.pular_espaco();
        if cursor.consumir('}') {
            cursor.pular_espaco();
            return fim_do_plano(&cursor).map(|()| pares.into_iter().collect());
        }
        if !cursor.consumir(',') {
            return Err("esperado ','".to_string());
        }
    }
}

fn fim_do_plano(cursor: &Cursor<'_>) -> Result<(), String> {
    if cursor.fim() {
        Ok(())
    } else {
        Err("conteúdo extra após objeto".to_string())
    }
}

/// O recorte plano nunca aceitou chave que exigisse escape.
///
/// A gramática compartilhada decodifica escapes, então a recusa passa a olhar a
/// chave **decodificada**. Consequência já registrada e aprovada: uma chave
/// escapada cujo texto decodificado é legal — `{"a\/b":1}` — passa a ser
/// aceita. Isso amplia aceitação; nenhum programa antes válido muda.
fn validar_chave_plana(chave: &str) -> Result<(), String> {
    if chave.contains('"') || chave.contains('\\') || chave.chars().any(char::is_control) {
        return Err("escapes em chave fora do recorte".to_string());
    }
    Ok(())
}

fn traduzir_erro_de_chave(causa: String) -> String {
    if causa.contains("não terminada") {
        "string de chave não terminada".to_string()
    } else {
        causa
    }
}

/// Serializa o recorte plano, com chaves em ordem e valores `u64` exatos.
///
/// Sem cast para `i64` em nenhum ponto: `u64::MAX` precisa sair
/// `18446744073709551615`, não `-1` nem um valor truncado.
pub fn serializar_plano_bombom(pares: &[(String, u64)]) -> Result<String, String> {
    let ordenados: BTreeMap<&str, u64> = pares
        .iter()
        .map(|(chave, valor)| (chave.as_str(), *valor))
        .collect();
    let mut partes = Vec::with_capacity(ordenados.len());
    for (chave, valor) in ordenados {
        validar_chave_plana(chave).map_err(|_| "chave exige escape fora do recorte".to_string())?;
        partes.push(format!("\"{chave}\":{valor}"));
    }
    Ok(format!("{{{}}}", partes.join(",")))
}
// @pinker-nav:end json.plano.projecao-legada

// @pinker-nav:start json.texto.serializacao
// @pinker-nav:domain dados
// @pinker-nav:layer semantica
// @pinker-nav:summary Serialização determinística da árvore: objetos saem em ordem de chave por construção do `BTreeMap`, strings escapam aspas, barra invertida e controles em forma canônica preservando UTF-8 multibyte cru, e números saem exatos. A regra de ordem é explícita e não herdada de iteração de host; a mesma regra já vigorava no emissor plano histórico, que ordenava as chaves.
/// Serializa a árvore a partir de `handle`, de forma determinística.
///
/// Objetos saem em ordem de chave. A regra é explícita porque determinismo foi
/// prometido — não é um efeito colateral observado uma vez e presumido estável.
pub fn serializar(handle: u64, tabela: &TabelaJson) -> Result<String, String> {
    let mut saida = String::new();
    serializar_em(handle, tabela, &mut saida, 0)?;
    Ok(saida)
}

fn serializar_em(
    handle: u64,
    tabela: &TabelaJson,
    saida: &mut String,
    profundidade: usize,
) -> Result<(), String> {
    if profundidade > LIMITE_PROFUNDIDADE {
        return Err(format!(
            "json inválido: profundidade acima do limite de {}",
            LIMITE_PROFUNDIDADE
        ));
    }
    let no = tabela
        .obter(handle)
        .ok_or_else(|| "handle de ValorJson inválido".to_string())?;
    match no {
        NoJson::Nulo => saida.push_str("null"),
        NoJson::Logica(true) => saida.push_str("true"),
        NoJson::Logica(false) => saida.push_str("false"),
        NoJson::Numero(valor) => saida.push_str(&valor.to_string()),
        NoJson::Verso(texto) => escrever_string(texto, saida),
        NoJson::Lista(itens) => {
            saida.push('[');
            for (indice, item) in itens.iter().enumerate() {
                if indice > 0 {
                    saida.push(',');
                }
                serializar_em(*item, tabela, saida, profundidade + 1)?;
            }
            saida.push(']');
        }
        NoJson::Objeto(membros) => {
            saida.push('{');
            for (indice, (chave, valor)) in membros.iter().enumerate() {
                if indice > 0 {
                    saida.push(',');
                }
                escrever_string(chave, saida);
                saida.push(':');
                serializar_em(*valor, tabela, saida, profundidade + 1)?;
            }
            saida.push('}');
        }
    }
    Ok(())
}

/// Escreve uma string JSON escapada.
///
/// UTF-8 multibyte sai cru, que é JSON válido e evita conversão desnecessária.
/// Controles saem na forma canônica curta quando existe, e em `\u00XX` quando
/// não existe.
fn escrever_string(texto: &str, saida: &mut String) {
    saida.push('"');
    for ch in texto.chars() {
        match ch {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            '\u{0008}' => saida.push_str("\\b"),
            '\u{000C}' => saida.push_str("\\f"),
            _ if (ch as u32) < 0x20 => {
                saida.push_str(&format!("\\u{:04x}", ch as u32));
            }
            _ => saida.push(ch),
        }
    }
    saida.push('"');
}
// @pinker-nav:end json.texto.serializacao

// @pinker-nav:start evidencia.json.modelo-unitario
// @pinker-nav:domain dados
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência unitária do modelo: handles monotônicos sem reuso, esgotamento sem wrap, nesting recursivo atravessado pelo mesmo mecanismo em duas árvores de formatos diferentes, domínio numérico exato com recusa de fração/expoente/magnitude, escapes e pares surrogate, chave duplicada recusada, profundidade limitada e serialização determinística por ordem de chave independente da ordem de inserção.
#[cfg(test)]
mod tests {
    use super::*;

    fn interpretar_ok(texto: &str) -> (u64, TabelaJson) {
        let mut tabela = TabelaJson::nova();
        let raiz = interpretar(texto, &mut tabela).expect("json válido");
        (raiz, tabela)
    }

    /// `default()` e `nova()` precisam produzir a MESMA tabela utilizável.
    #[test]
    fn default_e_nova_sao_equivalentes_e_utilizaveis() {
        let mut por_default = TabelaJson::default();
        let mut por_nova = TabelaJson::nova();
        assert_eq!(por_default.inserir(NoJson::Nulo), 1);
        assert_eq!(por_nova.inserir(NoJson::Nulo), 1);
    }

    #[test]
    fn handles_sao_monotonicos_e_nunca_reutilizados() {
        let mut tabela = TabelaJson::nova();
        let a = tabela.inserir(NoJson::Nulo);
        let b = tabela.inserir(NoJson::Logica(true));
        assert_eq!(a, 1, "zero não é identidade produzida");
        assert!(a < b);
        assert_eq!(tabela.retidos(), 2, "nada é removido");
        assert!(!std::hint::black_box(PoliticaValorJson::HANDLE_REUTILIZADO));
    }

    #[test]
    fn esgotamento_de_handle_nao_faz_wrap_nem_aba() {
        let mut tabela = TabelaJson::nova();
        let antigo = tabela.inserir(NoJson::Numero(11));
        tabela.proximo_handle = Some(u64::MAX);
        let ultimo = tabela.inserir(NoJson::Numero(22));
        assert_eq!(ultimo, u64::MAX);
        assert_eq!(tabela.proximo_handle, None);
        let esgotou = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tabela.inserir(NoJson::Numero(33));
        }));
        assert!(esgotou.is_err(), "esgotamento é falha de invariante");
        assert_eq!(tabela.obter(antigo), Some(&NoJson::Numero(11)));
        assert!(tabela.obter(0).is_none(), "wrap não reutiliza zero");
    }

    /// Duas árvores de formatos materialmente diferentes atravessadas pelo
    /// MESMO mecanismo. Se o nesting fosse enumeração de formatos, um dos dois
    /// exigiria helper próprio.
    #[test]
    fn nesting_recursivo_atravessa_duas_arvores_diferentes() {
        // objeto -> lista -> objeto
        let (raiz, tabela) = interpretar_ok(r#"{"a":[{"b":1}]}"#);
        let NoJson::Objeto(membros) = tabela.obter(raiz).unwrap() else {
            panic!("raiz deveria ser objeto");
        };
        let NoJson::Lista(itens) = tabela.obter(membros["a"]).unwrap() else {
            panic!("deveria ser lista");
        };
        let NoJson::Objeto(interno) = tabela.obter(itens[0]).unwrap() else {
            panic!("deveria ser objeto");
        };
        assert_eq!(tabela.obter(interno["b"]), Some(&NoJson::Numero(1)));

        // lista -> objeto -> lista
        let (raiz, tabela) = interpretar_ok(r#"[{"c":[7,8]}]"#);
        let NoJson::Lista(itens) = tabela.obter(raiz).unwrap() else {
            panic!("raiz deveria ser lista");
        };
        let NoJson::Objeto(membros) = tabela.obter(itens[0]).unwrap() else {
            panic!("deveria ser objeto");
        };
        let NoJson::Lista(internos) = tabela.obter(membros["c"]).unwrap() else {
            panic!("deveria ser lista");
        };
        assert_eq!(internos.len(), 2);
        assert_eq!(tabela.obter(internos[1]), Some(&NoJson::Numero(8)));
    }

    /// Matriz numérica diagonal: a MESMA gramática, dois domínios de projeção.
    ///
    /// O caso central é `i64::MAX + 1`: recusado pelo adulto e aceito pelo
    /// legado. Se alguém fizer a gramática decidir por `i64` antes da projeção,
    /// esta matriz quebra na diagonal em vez de perder faixa em silêncio.
    #[test]
    fn matriz_numerica_adulto_i64_versus_legado_u64() {
        // (json, adulto_ok, legado_ok, valor_legado_esperado)
        let casos: &[(&str, bool, bool, Option<u64>)] = &[
            // N1 zero
            (r#"{"x":0}"#, true, true, Some(0)),
            // N2 i64::MAX
            (
                r#"{"x":9223372036854775807}"#,
                true,
                true,
                Some(9223372036854775807),
            ),
            // N3 i64::MAX + 1 — a diagonal
            (
                r#"{"x":9223372036854775808}"#,
                false,
                true,
                Some(9223372036854775808),
            ),
            // N4 u64::MAX
            (
                r#"{"x":18446744073709551615}"#,
                false,
                true,
                Some(18446744073709551615),
            ),
            // N5 acima de u64::MAX — nenhuma das duas, e sem dar a volta
            (r#"{"x":18446744073709551616}"#, false, false, None),
            // N6 negativo
            (r#"{"x":-1}"#, true, false, None),
            // N7 i64::MIN
            (r#"{"x":-9223372036854775808}"#, true, false, None),
            // N8 abaixo de i64::MIN
            (r#"{"x":-9223372036854775809}"#, false, false, None),
            // N9 fração
            (r#"{"x":1.5}"#, false, false, None),
            // N10 expoente — mesmo resultando em inteiro
            (r#"{"x":1e3}"#, false, false, None),
        ];
        for (json, adulto_ok, legado_ok, valor_legado) in casos {
            let mut tabela = TabelaJson::nova();
            let adulto = interpretar(json, &mut tabela);
            assert_eq!(
                adulto.is_ok(),
                *adulto_ok,
                "adulto divergiu da matriz em {json}: {adulto:?}"
            );

            let legado = interpretar_plano_bombom(json);
            assert_eq!(
                legado.is_ok(),
                *legado_ok,
                "legado divergiu da matriz em {json}: {legado:?}"
            );
            if let Some(esperado) = valor_legado {
                let pares = legado.expect("legado deveria aceitar");
                assert_eq!(pares, vec![("x".to_string(), *esperado)]);
            }
        }
    }

    /// `u64::MAX` precisa sobreviver ao ciclo completo do recorte plano.
    #[test]
    fn legado_faz_round_trip_de_u64_max_sem_truncar_nem_trocar_sinal() {
        let origem = r#"{"x":18446744073709551615}"#;
        let pares = interpretar_plano_bombom(origem).expect("u64::MAX pertence ao recorte plano");
        assert_eq!(pares, vec![("x".to_string(), u64::MAX)]);
        let texto = serializar_plano_bombom(&pares).expect("emissão plana");
        assert_eq!(texto, origem, "u64::MAX não pode virar -1 nem truncar");
        assert!(!texto.contains('-'), "sinal apareceu do nada");
    }

    /// A emissão plana cobre a faixa inteira, não só o que cabe em `i64`.
    #[test]
    fn emissao_plana_cobre_a_faixa_u64_inteira() {
        let pares = vec![
            ("a".to_string(), 0u64),
            ("b".to_string(), 9223372036854775807),
            ("c".to_string(), 9223372036854775808),
            ("d".to_string(), u64::MAX),
        ];
        assert_eq!(
            serializar_plano_bombom(&pares).expect("emissão plana"),
            r#"{"a":0,"b":9223372036854775807,"c":9223372036854775808,"d":18446744073709551615}"#
        );
    }

    /// Uma gramática: as recusas estruturais históricas continuam recusando.
    #[test]
    fn legado_preserva_recusas_estruturais_historicas() {
        for (json, razao) in [
            ("[1,2,3]", "esperado '{'"),
            (r#"{"meta":{"x":1}}"#, "valor deve ser bombom sem sinal"),
            (r#"{"li\nha":1}"#, "escapes em chave fora do recorte"),
            (r#"{"a":1,"a":2}"#, "chave duplicada"),
            (r#"{"a":1} lixo"#, "conteúdo extra"),
            (r#"{"a":"texto"}"#, "valor deve ser bombom sem sinal"),
            (r#"{"a":true}"#, "valor deve ser bombom sem sinal"),
            (r#"{"a":null}"#, "valor deve ser bombom sem sinal"),
        ] {
            let erro =
                interpretar_plano_bombom(json).expect_err(&format!("deveria recusar {json}"));
            assert!(
                erro.contains(razao),
                "para {json} esperava conter {razao}, veio {erro}"
            );
        }
    }

    /// Ampliação aprovada: chave escapada que decodifica para chave legal passa.
    #[test]
    fn legado_aceita_chave_escapada_que_decodifica_para_chave_legal() {
        let pares = interpretar_plano_bombom(r#"{"a\/b":1}"#).expect("ampliação aprovada");
        assert_eq!(pares, vec![("a/b".to_string(), 1)]);
    }

    #[test]
    fn dominio_numerico_e_exato_e_recusa_o_resto() {
        let (raiz, tabela) = interpretar_ok("-9223372036854775808");
        assert_eq!(tabela.obter(raiz), Some(&NoJson::Numero(i64::MIN)));
        let (raiz, tabela) = interpretar_ok("9223372036854775807");
        assert_eq!(tabela.obter(raiz), Some(&NoJson::Numero(i64::MAX)));
        let (raiz, tabela) = interpretar_ok("0");
        assert_eq!(tabela.obter(raiz), Some(&NoJson::Numero(0)));

        for fora in [
            "9223372036854775808",  // i64::MAX + 1
            "-9223372036854775809", // i64::MIN - 1
            "1.5",
            "1e3",
            "01",
            "+1",
            ".5",
            "1.",
        ] {
            let mut tabela = TabelaJson::nova();
            assert!(
                interpretar(fora, &mut tabela).is_err(),
                "deveria recusar {fora}"
            );
        }
    }

    #[test]
    fn escapes_e_surrogates() {
        let (raiz, tabela) = interpretar_ok(r#""a\nbAé""#);
        assert_eq!(
            tabela.obter(raiz),
            Some(&NoJson::Verso("a\nbAé".to_string()))
        );
        // Par surrogate válido -> U+1F600
        let (raiz, tabela) = interpretar_ok(r#""😀""#);
        assert_eq!(
            tabela.obter(raiz),
            Some(&NoJson::Verso("\u{1F600}".to_string()))
        );
        // UTF-8 multibyte cru preservado
        let (raiz, tabela) = interpretar_ok("\"olá 😀\"");
        assert_eq!(
            tabela.obter(raiz),
            Some(&NoJson::Verso("olá 😀".to_string()))
        );

        for invalido in [
            r#""\ud83d""#,       // surrogate alto sem par
            r#""\udc00""#,       // surrogate baixo isolado
            r#""\ud83dx""#,      // par incompleto
            r#""\uZZZZ""#,       // não hexadecimal
            r#""\q""#,           // escape desconhecido
            "\"quebra\nlinha\"", // controle não escapado
            r#""sem fim"#,       // não terminada
        ] {
            let mut tabela = TabelaJson::nova();
            assert!(
                interpretar(invalido, &mut tabela).is_err(),
                "deveria recusar {invalido}"
            );
        }
    }

    #[test]
    fn chave_duplicada_e_recusada() {
        let mut tabela = TabelaJson::nova();
        let erro = interpretar(r#"{"a":1,"a":2}"#, &mut tabela).unwrap_err();
        assert!(erro.contains("chave duplicada"), "erro inesperado: {erro}");
    }

    #[test]
    fn lixo_apos_o_valor_e_recusado() {
        let mut tabela = TabelaJson::nova();
        let erro = interpretar("{} lixo", &mut tabela).unwrap_err();
        assert!(erro.contains("conteúdo extra"), "erro inesperado: {erro}");
    }

    /// O host precisa sobreviver a texto externo hostil.
    #[test]
    fn profundidade_excessiva_e_falha_recuperavel_e_nao_estouro_de_pilha() {
        let fundo = "[".repeat(LIMITE_PROFUNDIDADE + 10) + &"]".repeat(LIMITE_PROFUNDIDADE + 10);
        let mut tabela = TabelaJson::nova();
        let erro = interpretar(&fundo, &mut tabela).unwrap_err();
        assert!(erro.contains("profundidade"), "erro inesperado: {erro}");
    }

    /// Ordem de serialização é de chave, não de inserção.
    #[test]
    fn serializacao_e_deterministica_por_ordem_de_chave() {
        let mut tabela = TabelaJson::nova();
        let raiz_a = interpretar(r#"{"b":1,"a":2}"#, &mut tabela).unwrap();
        let raiz_b = interpretar(r#"{"a":2,"b":1}"#, &mut tabela).unwrap();
        assert_eq!(
            serializar(raiz_a, &tabela).unwrap(),
            r#"{"a":2,"b":1}"#,
            "ordem precisa ser de chave"
        );
        assert_eq!(
            serializar(raiz_a, &tabela).unwrap(),
            serializar(raiz_b, &tabela).unwrap(),
            "ordem de inserção não pode vazar para a saída"
        );
    }

    #[test]
    fn round_trip_preserva_estrutura_e_texto() {
        let origem = r#"{"lista":[1,-2,null,true,"x"],"objeto":{"k":"v"}}"#;
        let (raiz, tabela) = interpretar_ok(origem);
        assert_eq!(serializar(raiz, &tabela).unwrap(), origem);
    }

    #[test]
    fn tipos_sao_classificados_exaustivamente() {
        for (texto, esperado) in [
            ("{}", TipoJson::Objeto),
            ("[]", TipoJson::Lista),
            (r#""s""#, TipoJson::Verso),
            ("1", TipoJson::Numero),
            ("true", TipoJson::Logica),
            ("null", TipoJson::Nulo),
        ] {
            let (raiz, tabela) = interpretar_ok(texto);
            assert_eq!(tabela.obter(raiz).unwrap().tipo(), esperado);
        }
    }

    /// A ordem de declaração é o discriminante lido pela IR.
    #[test]
    fn discriminantes_seguem_a_ordem_de_declaracao() {
        for (indice, nome) in VARIANTES.iter().enumerate() {
            let tipo = match *nome {
                "Objeto" => TipoJson::Objeto,
                "Lista" => TipoJson::Lista,
                "Verso" => TipoJson::Verso,
                "Numero" => TipoJson::Numero,
                "Logica" => TipoJson::Logica,
                "Nulo" => TipoJson::Nulo,
                outro => panic!("variante inesperada: {outro}"),
            };
            assert_eq!(tipo.discriminante(), indice as u64);
            assert_eq!(tipo.nome(), *nome);
        }
    }
}
// @pinker-nav:end evidencia.json.modelo-unitario
