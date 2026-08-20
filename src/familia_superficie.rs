//! Superfície pública das famílias built-in importáveis.
//!
//! A família deixou de ser um predicado sobre um nome de módulo e passou a ser
//! um mecanismo: `trazer arquivo;` habilita a forma qualificada
//! `arquivo.<membro>(...)`, e `trazer arquivo.<membro>;` habilita a forma bare
//! `<membro>(...)`. Ambas resolvem, no parser, para a **identidade executiva já
//! existente** — nenhuma camada a jusante aprende o que é uma família.

use crate::falha_operacional::{self, OperacaoFalivel};

// @pinker-nav:start familia.superficie.registro
// @pinker-nav:domain importacoes
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única da superfície por família da Parte G: `FAMILIAS` fixa quais famílias built-in são importáveis, `EXPORTACOES` liga cada par `(família, membro)` à identidade executiva que já existe, e `resolver` é o único lugar onde essa ligação é consultada — pelo parser, que canonicaliza a chamada antes de qualquer camada a jusante, e pela semântica, que diagnostica o import. O registro declara apenas ligação: assinatura, aridade, modelo de falha, política de follow e símbolo de runtime continuam sendo ditos por `semantic`, `falha_operacional` e `backend_s`, e nenhum deles é repetido aqui. Para superfície falível a identidade é `OperacaoFalivel`, nunca o texto do nome público — inclusive a grafia do membro, quando homônima, é derivada da autoridade e não escrita neste arquivo.

/// Famílias built-in que `trazer` aceita.
///
/// Lista fechada e única: `semantic` consulta esta constante em vez de manter
/// uma segunda cópia. Uma família sem exportações continua importável e
/// continua sendo um no-op de visibilidade — o que ela não faz é resolver
/// membro nenhum.
pub const FAMILIAS: &[&str] = &[
    "tempo", "ambiente", "acaso", "texto", "arquivo", "caminho", "processo",
];

/// Endereço da identidade executiva de um membro.
///
/// Não é um nome novo: é o modo de chegar ao nome que já existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentidadeCanonica {
    /// A intrínseca histórica cuja identidade **é** o próprio texto.
    ///
    /// Endereçar por texto é a dívida que a #477 deve resolver; hoje não existe
    /// outro endereço para essas superfícies. O custo é pago uma vez por
    /// membro, aqui, e não uma vez por camada de pipeline.
    Historica(&'static str),
    /// A superfície falível da Parte B, endereçada pela operação.
    ///
    /// O nome público sai de `falha_operacional` e não é repetido neste
    /// arquivo — invariante fixado por
    /// `nome_publico_de_superficie_falivel_existe_so_na_autoridade`.
    Falivel(OperacaoFalivel),
}

impl IdentidadeCanonica {
    /// Nome público da identidade executiva, sempre lido da autoridade que o
    /// declara.
    pub fn nome_publico(&self) -> &'static str {
        match self {
            IdentidadeCanonica::Historica(nome) => nome,
            IdentidadeCanonica::Falivel(operacao) => {
                falha_operacional::superficie_por_operacao(*operacao)
                    .expect("operação falível registrada na autoridade")
                    .intrinseca
            }
        }
    }

    /// A identidade pertence à autoridade de superfícies falíveis?
    pub fn e_falivel(&self) -> bool {
        matches!(self, IdentidadeCanonica::Falivel(_))
    }
}

/// Uma exportação de família: a grafia pública e para onde ela resolve.
///
/// Exportação não é posse. Duas entradas podem apontar para a mesma identidade
/// executiva; o que é único é a identidade, não a linha da tabela.
#[derive(Debug, Clone, Copy)]
pub struct Exportacao {
    /// Família que exporta o membro.
    pub familia: &'static str,
    /// Grafia do membro sob a família.
    ///
    /// `None` significa "escreve-se exatamente como a identidade canônica" — é
    /// o que mantém membros homônimos de superfície falível fora deste arquivo.
    grafia: Option<&'static str>,
    /// Identidade executiva para a qual o membro resolve.
    pub identidade: IdentidadeCanonica,
}

impl Exportacao {
    /// Grafia pública do membro sob a família.
    pub fn membro(&self) -> &'static str {
        match self.grafia {
            Some(grafia) => grafia,
            None => self.identidade.nome_publico(),
        }
    }
}

/// Constrói uma exportação cuja grafia difere da identidade canônica.
const fn exportar(
    familia: &'static str,
    grafia: &'static str,
    identidade: IdentidadeCanonica,
) -> Exportacao {
    Exportacao {
        familia,
        grafia: Some(grafia),
        identidade,
    }
}

/// Constrói uma exportação cuja grafia **é** a da identidade canônica.
const fn exportar_homonima(familia: &'static str, identidade: IdentidadeCanonica) -> Exportacao {
    Exportacao {
        familia,
        grafia: None,
        identidade,
    }
}

const ARQUIVO: &str = "arquivo";
const CAMINHO: &str = "caminho";

/// A superfície aprovada, em ordem de declaração.
///
/// Cada linha é `(família, grafia, identidade)` e mais nada. Toda pergunta
/// sobre comportamento — o que a operação faz, o que aceita, se aborta ou
/// devolve `Resultado`, se segue symlink — continua sendo respondida pela
/// camada que já a respondia antes desta tabela existir.
pub const EXPORTACOES: &[Exportacao] = &[
    // ----- família `arquivo` -----
    exportar_homonima(ARQUIVO, IdentidadeCanonica::Historica("abrir")),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::Historica("fechar")),
    exportar(
        ARQUIVO,
        "ler_bombom",
        IdentidadeCanonica::Historica("ler_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "ler_verso",
        IdentidadeCanonica::Historica("ler_verso_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_verso",
        IdentidadeCanonica::Historica("ler_arquivo_verso"),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_ou",
        IdentidadeCanonica::Historica("arquivo_ou"),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_resultado",
        IdentidadeCanonica::Falivel(OperacaoFalivel::LerArquivoPorCaminho),
    ),
    exportar(
        ARQUIVO,
        "escrever_bombom",
        IdentidadeCanonica::Historica("escrever"),
    ),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::Historica("escrever_verso")),
    exportar(
        ARQUIVO,
        "criar",
        IdentidadeCanonica::Historica("criar_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "truncar",
        IdentidadeCanonica::Historica("truncar_arquivo"),
    ),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::Historica("abrir_anexo")),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::Historica("anexar_verso")),
    exportar(
        ARQUIVO,
        "copiar",
        IdentidadeCanonica::Historica("copiar_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "renomear",
        IdentidadeCanonica::Historica("renomear_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "sha256",
        IdentidadeCanonica::Falivel(OperacaoFalivel::HashArquivo),
    ),
    // ----- família `caminho` -----
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("caminho_existe")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("e_arquivo")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("e_diretorio")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("juntar_caminho")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("tamanho_arquivo")),
    // O irmão comportamental é `tamanho_arquivo`, não `e_arquivo`: mesma família
    // de runtime, mesmo FOLLOW, mesma fatalidade e mesma exigência de arquivo
    // regular. A grafia nomeia o sujeito que a operação de fato exige.
    exportar(
        CAMINHO,
        "arquivo_vazio",
        IdentidadeCanonica::Historica("e_vazio"),
    ),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("criar_diretorio")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("remover_arquivo")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("remover_diretorio")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::Historica("diretorio_atual")),
    // As três superfícies adultas abaixo são homônimas da própria identidade
    // canônica, e essa identidade é falível: a grafia do membro sai da
    // autoridade em vez de ser escrita aqui. Nenhuma família pública nova é
    // aberta nesta fase — o runtime nomear `diretorio` e `entrada` é fato
    // registrado, não autorização.
    exportar_homonima(
        CAMINHO,
        IdentidadeCanonica::Falivel(OperacaoFalivel::EnumerarDiretorio),
    ),
    exportar_homonima(
        CAMINHO,
        IdentidadeCanonica::Falivel(OperacaoFalivel::ClassificarEntrada),
    ),
    exportar_homonima(
        CAMINHO,
        IdentidadeCanonica::Falivel(OperacaoFalivel::MedirEntrada),
    ),
];

/// A família é built-in importável?
pub fn familia_conhecida(familia: &str) -> bool {
    FAMILIAS.contains(&familia)
}

/// Exportação de `(família, membro)`, se existir.
pub fn exportacao(familia: &str, membro: &str) -> Option<&'static Exportacao> {
    EXPORTACOES
        .iter()
        .find(|exportacao| exportacao.familia == familia && exportacao.membro() == membro)
}

/// `(família, membro)` → nome da identidade executiva canônica.
///
/// É o único ponto de resolução do mecanismo. Devolve `None` quando a família
/// não exporta o membro — inclusive quando a família nem existe.
pub fn resolver(familia: &str, membro: &str) -> Option<&'static str> {
    exportacao(familia, membro).map(|exportacao| exportacao.identidade.nome_publico())
}

/// O membro resolve para uma superfície falível da Parte B?
pub fn membro_e_falivel(familia: &str, membro: &str) -> bool {
    exportacao(familia, membro).is_some_and(|exportacao| exportacao.identidade.e_falivel())
}

/// Membros exportados pela família, em ordem de declaração.
pub fn membros_da_familia(familia: &str) -> Vec<&'static str> {
    EXPORTACOES
        .iter()
        .filter(|exportacao| exportacao.familia == familia)
        .map(Exportacao::membro)
        .collect()
}

/// `trazer familia.membro;` é válido?
pub fn import_seletivo_valido(familia: &str, membro: &str) -> bool {
    resolver(familia, membro).is_some()
}

/// `familia.membro(...)` é válido, supondo a família importada e não sombreada?
pub fn forma_qualificada_valida(familia: &str, membro: &str) -> bool {
    resolver(familia, membro).is_some()
}

/// Diagnóstico de membro inexistente, com a lista real da família.
///
/// Existe para que a mensagem venha da autoridade que conhece os membros, e
/// não de cada camada que precise recusar um.
pub fn membro_inexistente(familia: &str, membro: &str) -> String {
    let membros = membros_da_familia(familia);
    if membros.is_empty() {
        return format!(
            "membro '{}' não existe na família '{}'; a família '{}' não exporta membros nesta fase",
            membro, familia, familia
        );
    }
    format!(
        "membro '{}' não existe na família '{}'; membros desta fase: {}",
        membro,
        familia,
        membros
            .iter()
            .map(|nome| format!("'{}'", nome))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Diagnóstico de uso qualificado sem o import da família.
pub fn familia_nao_importada(familia: &str, membro: &str) -> String {
    format!(
        "família '{}' não foi importada neste arquivo; escreva 'trazer {};' antes de usar '{}.{}'",
        familia, familia, familia, membro
    )
}

/// Lista das famílias importáveis, para diagnóstico.
pub fn familias_disponiveis() -> String {
    FAMILIAS
        .iter()
        .map(|familia| format!("'{}'", familia))
        .collect::<Vec<_>>()
        .join(", ")
}
// @pinker-nav:end familia.superficie.registro
