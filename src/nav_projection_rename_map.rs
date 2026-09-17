//! Trama Pinker — mapa explícito de renomeação corrente → histórica (#685).
//!
//! A projeção estável mede cada região por oito campos, e três deles —
//! `key`, `domain` e `layer` — são a identidade pela qual a medida congelada
//! reconhece a região. Uma renomeação corrente autorizada apaga essa
//! identidade, e nenhuma ferramenta pode adivinhar que duas grafias são a mesma
//! coisa: semelhança textual, posição no arquivo, hash igual e caminho igual são
//! todos compatíveis com duas regiões diferentes.
//!
//! Este módulo é a superfície pela qual o operador **declara** a relação. Ele
//! não é autoridade persistente: o arquivo é passado por caminho na invocação,
//! vive fora do acervo de projeções e não é lido de lugar nenhum por omissão.
//! Ele descreve uma reconstrução, não uma segunda nomenclatura canônica.
//!
//! # Direção
//!
//! ```text
//! current     = grafia corrente, já normalizada no catálogo
//! historical  = grafia que a autoridade de reconstrução espera
//! ```
//!
//! A direção não tem exceção e os dois lados nunca compartilham nome de campo.
//! Para uma região que participa de alguma projeção congelada, `historical_*` é
//! o valor que a medida exige de volta.
//!
//! # Identidade e summary
//!
//! `summary` também é medido pela projeção estável, mas não é identidade: a
//! região continua sendo selecionada por `current_key` depois de restaurado. O
//! mapa declara os dois tipos de restauração na mesma entrada, e quem consome
//! precisa distingui-los — restaurar identidade tira a região do alcance das
//! outras regras, restaurar summary não. Para uma região que nenhuma projeção
//! histórica contém, a única autoridade que a nomeia é o seletor de exclusão da
//! própria receita, e `historical_key` é a grafia que aquele seletor usa.

// @pinker-nav:start trama.snapshots.rename-map
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Explicit operator-declared map from a current region to the historical identity and summary a frozen reconstruction expects: parses its own strict TOML subset through the shared reader, keeps current and historical sides under distinct field names so direction cannot be read backwards, versions the format so schema 1 keeps its identity-only meaning and only schema 2 may declare a summary pair, rejects a repeated current or historical key, an entry that declares half a pair, an entry that restores a value the region already has and an entry that declares nothing to restore, separates restoring identity from restoring summary because only the first changes which region a rule selects, offers lookup both by historical key and by current key so a rename that moved only domain, layer or summary is still found by a rule whose selector never changed, and publishes a canonical fingerprint over every declared field so the map binds into the reconciliation plan digest.
use crate::nav_projection_snapshot::{
    optional_text, parse_raw_with_array, reject_unknown, require_text, Table,
};
use std::collections::BTreeSet;

/// Primeira versão do formato do mapa de renomeação: identidade apenas —
/// `historical_key`, `historical_domain` e `historical_layer`.
pub const RENAME_MAP_SCHEMA_V1: u64 = 1;

/// Segunda versão: acrescenta o par de summary (#693).
///
/// `summary` participa da projeção estável como `key`, `domain` e `layer`, mas
/// não é identidade: restaurá-lo não muda qual região a regra seleciona. A
/// capacidade nasce numa versão nova porque um mapa schema 1 não podia
/// declará-la, e aceitar o campo novo sob a versão antiga faria um mapa velho
/// significar uma coisa que seu autor não escreveu.
pub const RENAME_MAP_SCHEMA_V2: u64 = 2;

/// Versão máxima aceita do formato do mapa de renomeação.
pub const RENAME_MAP_SCHEMA: u64 = RENAME_MAP_SCHEMA_V2;

/// Uma relação declarada entre a identidade corrente e a identidade histórica
/// de uma mesma região.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEntry {
    /// Chave corrente. É o seletor: precisa existir exatamente uma vez no
    /// catálogo corrente.
    pub current_key: String,
    /// Chave que a autoridade de reconstrução espera, quando ela difere.
    pub historical_key: Option<String>,
    /// Domínio corrente declarado, usado como guarda contra mapa envelhecido.
    pub current_domain: Option<String>,
    /// Domínio histórico restaurado.
    pub historical_domain: Option<String>,
    /// Camada corrente declarada, usada como guarda contra mapa envelhecido.
    pub current_layer: Option<String>,
    /// Camada histórica restaurada.
    pub historical_layer: Option<String>,
    /// Summary corrente declarado. É guarda exata sobre o catálogo corrente, e
    /// nunca seletor: a região continua sendo escolhida por `current_key`.
    pub current_summary: Option<String>,
    /// Summary imediatamente anterior à renomeação corrente, restaurado.
    ///
    /// É o valor que a reconstrução espera **agora**, não necessariamente o
    /// destino histórico final: uma regra existente pode já encadear uma
    /// transição mais antiga, e o destino dela é preservado.
    pub historical_summary: Option<String>,
}

impl RenameEntry {
    /// Verdadeiro quando a entrada restaura a identidade pela qual a projeção
    /// reconhece a região: chave, domínio ou camada.
    ///
    /// Restaurar `summary` não entra: o campo participa da medida e não da
    /// seleção, então uma entrada só de summary não tira a região do alcance de
    /// nenhuma outra regra.
    pub fn restores_identity(&self) -> bool {
        self.historical_key.is_some()
            || self.historical_domain.is_some()
            || self.historical_layer.is_some()
    }
}

impl RenameEntry {
    /// Forma canônica de uma entrada, usada na impressão digital.
    fn canonical(&self) -> String {
        let campo = |nome: &str, valor: &Option<String>| match valor {
            Some(v) => format!("{nome}={v};"),
            None => format!("{nome}=;"),
        };
        format!(
            "current_key={};{}{}{}{}{}{}{}",
            self.current_key,
            campo("historical_key", &self.historical_key),
            campo("current_domain", &self.current_domain),
            campo("historical_domain", &self.historical_domain),
            campo("current_layer", &self.current_layer),
            campo("historical_layer", &self.historical_layer),
            campo("current_summary", &self.current_summary),
            campo("historical_summary", &self.historical_summary),
        )
    }
}

/// O mapa inteiro, em ordem determinística por chave corrente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameMap {
    pub schema: u64,
    pub entries: Vec<RenameEntry>,
}

impl RenameMap {
    /// Entrada cuja identidade histórica é a grafia indicada.
    ///
    /// É a busca que o reconciliador faz: uma regra existente nomeia a grafia
    /// histórica, e o mapa diz qual região corrente responde por ela.
    pub fn by_historical_key(&self, key: &str) -> Option<&RenameEntry> {
        self.entries
            .iter()
            .find(|entry| entry.historical_key.as_deref() == Some(key))
    }

    /// Entrada que nomeia a região corrente indicada.
    ///
    /// É a busca complementar: a chave da regra continua resolvendo no catálogo
    /// e mesmo assim o operador declarou que a identidade histórica daquela
    /// mesma região é outra. Sem esta busca, uma renomeação só de `domain` ou
    /// `layer` ficaria invisível para o reconciliador.
    pub fn by_current_key(&self, key: &str) -> Option<&RenameEntry> {
        self.entries.iter().find(|entry| entry.current_key == key)
    }

    /// Impressão digital canônica do mapa.
    ///
    /// Existe para que o mapa participe do digest do plano: dois mapas
    /// diferentes nunca autorizam o mesmo plano, mesmo que por acaso
    /// produzissem os mesmos bytes de receita.
    pub fn fingerprint(&self) -> String {
        let mut texto = format!("rename-map/{}\n", self.schema);
        for entry in &self.entries {
            texto.push_str(&entry.canonical());
            texto.push('\n');
        }
        pinker_sha256_contract::sha256_hex(texto.as_bytes())
    }
}

const ROOT_KEYS: [&str; 1] = ["schema"];
const ENTRY_KEYS: [&str; 8] = [
    "current_key",
    "historical_key",
    "current_domain",
    "historical_domain",
    "current_layer",
    "historical_layer",
    "current_summary",
    "historical_summary",
];

/// Campos que só existem a partir do schema 2.
const V2_ONLY_KEYS: [&str; 2] = ["current_summary", "historical_summary"];

/// Interpreta o texto de um mapa de renomeação. Não toca no filesystem.
///
/// Reusa o mesmo subconjunto TOML do acervo de projeções: um segundo leitor com
/// outra tolerância a escape, chave duplicada e lixo residual seria uma segunda
/// gramática para a mesma família de artefatos.
pub fn parse_rename_map(text: &str) -> Result<RenameMap, String> {
    let raw = parse_raw_with_array(text, "rename").map_err(|erro| erro.to_string())?;
    if raw.reconstruction.is_some() || raw.measures.is_some() {
        return Err(
            "o mapa de renomeação não tem seção de reconstrução nem de medidas".to_string(),
        );
    }
    reject_unknown(&raw.root, &ROOT_KEYS, "").map_err(|erro| erro.to_string())?;
    let schema = match raw.root.get("schema").and_then(|valor| valor.as_integer()) {
        Some(valor) => valor,
        None => return Err("campo 'schema' ausente ou não inteiro".to_string()),
    };
    if schema != RENAME_MAP_SCHEMA_V1 && schema != RENAME_MAP_SCHEMA_V2 {
        return Err(format!(
            "schema {schema} desconhecido para mapa de renomeação; este formato aceita {RENAME_MAP_SCHEMA_V1} e {RENAME_MAP_SCHEMA_V2}"
        ));
    }
    if raw.rules.is_empty() {
        return Err("o mapa de renomeação precisa declarar ao menos uma entrada".to_string());
    }

    let mut entries = Vec::with_capacity(raw.rules.len());
    for (index, table) in raw.rules.iter().enumerate() {
        entries.push(build_entry(table, index, schema)?);
    }

    // Duas entradas para a mesma região corrente, ou duas entradas reivindicando
    // a mesma identidade histórica, deixariam a reconstrução ambígua. Nenhuma
    // das duas é resolvível por escolha: as duas recusam.
    let mut correntes: BTreeSet<&str> = BTreeSet::new();
    let mut historicas: BTreeSet<&str> = BTreeSet::new();
    for entry in &entries {
        if !correntes.insert(entry.current_key.as_str()) {
            return Err(format!(
                "chave corrente '{}' declarada em duas entradas",
                entry.current_key
            ));
        }
        if let Some(historica) = &entry.historical_key {
            if !historicas.insert(historica.as_str()) {
                return Err(format!(
                    "identidade histórica '{historica}' reivindicada por duas entradas"
                ));
            }
        }
    }
    // Uma chave que é corrente numa entrada e histórica em outra descreveria uma
    // troca de nomes cuja ordem de aplicação decidiria o resultado. Recusa.
    for entry in &entries {
        if let Some(historica) = &entry.historical_key {
            if correntes.contains(historica.as_str()) {
                return Err(format!(
                    "'{historica}' aparece como identidade histórica e como chave corrente no mesmo mapa"
                ));
            }
        }
    }

    entries.sort_by(|a, b| a.current_key.cmp(&b.current_key));
    Ok(RenameMap { schema, entries })
}

fn build_entry(table: &Table, index: usize, schema: u64) -> Result<RenameEntry, String> {
    let scope = format!("rename[{index}].");
    reject_unknown(table, &ENTRY_KEYS, &scope).map_err(|erro| erro.to_string())?;
    // Um mapa schema 1 não podia declarar summary, então um campo de summary
    // nele não é um mapa novo: é um mapa antigo que ganhou um campo que sua
    // versão não define. A recusa é nomeada — "chave desconhecida" diria que o
    // campo não existe em versão nenhuma, que é outra coisa.
    if schema < RENAME_MAP_SCHEMA_V2 {
        for campo in V2_ONLY_KEYS {
            if table.get(campo).is_some() {
                return Err(format!(
                    "{scope}{campo} exige schema {RENAME_MAP_SCHEMA_V2}; o mapa declara schema {schema}"
                ));
            }
        }
    }
    let current_key = require_text(table, "current_key", &scope).map_err(|e| e.to_string())?;
    if current_key.is_empty() {
        return Err(format!("{scope}current_key vazio"));
    }
    let leitura = |campo: &str| -> Result<Option<String>, String> {
        optional_text(table, campo, &scope).map_err(|erro| erro.to_string())
    };
    let historical_key = leitura("historical_key")?;
    let current_domain = leitura("current_domain")?;
    let historical_domain = leitura("historical_domain")?;
    let current_layer = leitura("current_layer")?;
    let historical_layer = leitura("historical_layer")?;
    let current_summary = leitura("current_summary")?;
    let historical_summary = leitura("historical_summary")?;

    for (campo, valor) in [
        ("historical_key", &historical_key),
        ("current_domain", &current_domain),
        ("historical_domain", &historical_domain),
        ("current_layer", &current_layer),
        ("historical_layer", &historical_layer),
        ("current_summary", &current_summary),
        ("historical_summary", &historical_summary),
    ] {
        if valor.as_deref() == Some("") {
            return Err(format!("{scope}{campo} vazio"));
        }
    }

    // Guarda e destino vêm juntos, como em `expect_file`/`to_file`: restaurar
    // sem declarar a origem seria mutação sem precondição, e declarar a origem
    // sem destino não restaura nada.
    for (guarda, destino, nome) in [
        (&current_domain, &historical_domain, "domain"),
        (&current_layer, &historical_layer, "layer"),
        (&current_summary, &historical_summary, "summary"),
    ] {
        if guarda.is_some() != destino.is_some() {
            return Err(format!(
                "{scope}current_{nome} e historical_{nome} precisam vir juntos"
            ));
        }
        if let (Some(atual), Some(historico)) = (guarda, destino) {
            if atual == historico {
                return Err(format!(
                    "{scope}historical_{nome} repete o valor corrente e não restaura nada"
                ));
            }
        }
    }
    if historical_key.as_ref() == Some(&current_key) {
        return Err(format!(
            "{scope}historical_key repete a chave corrente e não restaura nada"
        ));
    }
    if historical_key.is_none()
        && historical_domain.is_none()
        && historical_layer.is_none()
        && historical_summary.is_none()
    {
        return Err(format!("{scope}não declara nenhuma restauração"));
    }

    Ok(RenameEntry {
        current_key,
        historical_key,
        current_domain,
        historical_domain,
        current_layer,
        historical_layer,
        current_summary,
        historical_summary,
    })
}
// @pinker-nav:end trama.snapshots.rename-map
