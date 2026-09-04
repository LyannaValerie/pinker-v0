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
// @pinker-nav:summary Autoridade única da superfície pública de intrínsecas: `FAMILIAS` fixa os quinze módulos built-in importáveis, `EXPORTACOES` liga cada par `(módulo, membro)` à identidade executiva que já existe, e `resolver` é o único lugar onde essa ligação é consultada — pelo parser, que canonicaliza a chamada antes de qualquer camada a jusante, e pela semântica, que diagnostica o import. Depois da #505 este registro não é mais um subconjunto da superfície pública: ele **é** a superfície pública, e toda intrínseca pública pertence a exatamente um módulo. O registro declara apenas ligação: assinatura, aridade, modelo de falha, política de follow e símbolo de runtime continuam sendo ditos por `semantic`, `falha_operacional` e `backend_s`, e nenhum deles é repetido aqui. A identidade é endereçada por `OperacaoFalivel` na superfície falível e pela grafia canônica no resto, e quem traduz grafia canônica em identidade é `intrinsics::identity`.

/// Módulos built-in que `trazer` aceita.
///
/// Lista fechada e única: `semantic` e `parser` consultam esta constante em vez
/// de manter uma segunda cópia. Depois da #505 ela não é mais um subconjunto da
/// superfície pública — ela **é** a superfície pública, e toda intrínseca
/// pública pertence a exatamente um destes módulos.
pub const FAMILIAS: &[&str] = &[
    "acaso",
    "ambiente",
    "arquivo",
    "assertiva",
    "caminho",
    "csv",
    "entrada",
    "integridade",
    "json",
    "lista",
    "mapa",
    "memoria",
    "processo",
    "tempo",
    "texto",
];

const ACASO: &str = "acaso";
const AMBIENTE: &str = "ambiente";
const ARQUIVO: &str = "arquivo";
const ASSERTIVA: &str = "assertiva";
const CAMINHO: &str = "caminho";
const CSV: &str = "csv";
const ENTRADA: &str = "entrada";
const INTEGRIDADE: &str = "integridade";
const JSON: &str = "json";
const LISTA: &str = "lista";
const MAPA: &str = "mapa";
const MEMORIA: &str = "memoria";
const PROCESSO: &str = "processo";
const TEMPO: &str = "tempo";
const TEXTO: &str = "texto";

/// Endereço da identidade executiva de um membro.
///
/// Não é um nome novo: é o modo de chegar ao nome que já existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentidadeCanonica {
    /// A identidade endereçada pela sua **grafia canônica**.
    ///
    /// Cobre toda superfície cuja identidade é o próprio texto — histórica,
    /// acessor JSON, acessor SHA-256 e acessor de processo. Quem traduz a
    /// grafia canônica na identidade real é `intrinsics::identity`, autoridade
    /// única dessa relação; este registro apenas a endereça. Endereçar por
    /// texto continua sendo a dívida que a #477 deve resolver.
    PorGrafia(&'static str),
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
            IdentidadeCanonica::PorGrafia(nome) => nome,
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

/// A superfície pública inteira, em ordem de módulo e de declaração.
///
/// Cada linha é `(módulo, grafia, identidade)` e mais nada. Toda pergunta sobre
/// comportamento — o que a operação faz, o que aceita, se aborta ou devolve
/// `Resultado`, se segue symlink — continua sendo respondida pela camada que já
/// a respondia antes desta tabela existir.
pub const EXPORTACOES: &[Exportacao] = &[
    // ----- módulo `acaso` -----
    exportar(
        ACASO,
        "criar",
        IdentidadeCanonica::PorGrafia("aleatorio_criar"),
    ),
    exportar(
        ACASO,
        "entre",
        IdentidadeCanonica::PorGrafia("aleatorio_entre"),
    ),
    exportar(
        ACASO,
        "proximo",
        IdentidadeCanonica::PorGrafia("aleatorio_proximo"),
    ),
    // ----- módulo `ambiente` -----
    exportar(
        AMBIENTE,
        "variavel_ou",
        IdentidadeCanonica::PorGrafia("ambiente_ou"),
    ),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("argumento")),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("argumento_ou")),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("buscar_contexto")),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("pedir_argumento")),
    exportar_homonima(
        AMBIENTE,
        IdentidadeCanonica::PorGrafia("quantos_argumentos"),
    ),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("tem_argumento")),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("tem_chave")),
    exportar_homonima(AMBIENTE, IdentidadeCanonica::PorGrafia("tem_flag")),
    // ----- módulo `arquivo` -----
    exportar_homonima(ARQUIVO, IdentidadeCanonica::PorGrafia("abrir")),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::PorGrafia("abrir_anexo")),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::PorGrafia("anexar_verso")),
    exportar(
        ARQUIVO,
        "copiar",
        IdentidadeCanonica::PorGrafia("copiar_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "criar",
        IdentidadeCanonica::PorGrafia("criar_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "escrever_bombom",
        IdentidadeCanonica::PorGrafia("escrever"),
    ),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::PorGrafia("escrever_verso")),
    exportar_homonima(ARQUIVO, IdentidadeCanonica::PorGrafia("fechar")),
    exportar(
        ARQUIVO,
        "ler_bombom",
        IdentidadeCanonica::PorGrafia("ler_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_ou",
        IdentidadeCanonica::PorGrafia("arquivo_ou"),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_resultado",
        IdentidadeCanonica::Falivel(OperacaoFalivel::LerArquivoPorCaminho),
    ),
    exportar(
        ARQUIVO,
        "ler_caminho_verso",
        IdentidadeCanonica::PorGrafia("ler_arquivo_verso"),
    ),
    exportar(
        ARQUIVO,
        "ler_verso",
        IdentidadeCanonica::PorGrafia("ler_verso_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "renomear",
        IdentidadeCanonica::PorGrafia("renomear_arquivo"),
    ),
    exportar(
        ARQUIVO,
        "truncar",
        IdentidadeCanonica::PorGrafia("truncar_arquivo"),
    ),
    // ----- módulo `assertiva` -----
    exportar_homonima(ASSERTIVA, IdentidadeCanonica::PorGrafia("afirmar")),
    // ----- módulo `caminho` -----
    exportar(
        CAMINHO,
        "existe",
        IdentidadeCanonica::PorGrafia("caminho_existe"),
    ),
    exportar(
        CAMINHO,
        "juntar",
        IdentidadeCanonica::PorGrafia("juntar_caminho"),
    ),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("e_arquivo")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("e_diretorio")),
    exportar(
        CAMINHO,
        "arquivo_vazio",
        IdentidadeCanonica::PorGrafia("e_vazio"),
    ),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("tamanho_arquivo")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("criar_diretorio")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("remover_arquivo")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("remover_diretorio")),
    exportar_homonima(CAMINHO, IdentidadeCanonica::PorGrafia("diretorio_atual")),
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
    // ----- módulo `csv` -----
    exportar(
        CSV,
        "emitir_linha_bombom",
        IdentidadeCanonica::PorGrafia("emitir_linha_csv_bombom"),
    ),
    exportar(
        CSV,
        "ler_linha_bombom",
        IdentidadeCanonica::PorGrafia("ler_linha_csv_bombom"),
    ),
    // ----- módulo `entrada` -----
    exportar_homonima(ENTRADA, IdentidadeCanonica::PorGrafia("ouvir")),
    exportar_homonima(ENTRADA, IdentidadeCanonica::PorGrafia("ouvir_verso")),
    exportar_homonima(ENTRADA, IdentidadeCanonica::PorGrafia("ouvir_verso_ou")),
    // ----- módulo `integridade` -----
    exportar_homonima(
        INTEGRIDADE,
        IdentidadeCanonica::Falivel(OperacaoFalivel::HashArquivo),
    ),
    exportar_homonima(INTEGRIDADE, IdentidadeCanonica::PorGrafia("sha256_verso")),
    // ----- módulo `json` -----
    exportar(JSON, "tipo", IdentidadeCanonica::PorGrafia("json_tipo")),
    exportar(
        JSON,
        "como_verso",
        IdentidadeCanonica::PorGrafia("json_verso"),
    ),
    exportar(
        JSON,
        "como_numero",
        IdentidadeCanonica::PorGrafia("json_numero"),
    ),
    exportar(
        JSON,
        "como_logica",
        IdentidadeCanonica::PorGrafia("json_logica"),
    ),
    exportar(
        JSON,
        "lista_obter",
        IdentidadeCanonica::PorGrafia("json_lista_obter"),
    ),
    exportar(
        JSON,
        "lista_tamanho",
        IdentidadeCanonica::PorGrafia("json_lista_tamanho"),
    ),
    exportar(
        JSON,
        "objeto_obter",
        IdentidadeCanonica::PorGrafia("json_objeto_obter"),
    ),
    exportar(
        JSON,
        "objeto_tem",
        IdentidadeCanonica::PorGrafia("json_objeto_tem"),
    ),
    exportar(
        JSON,
        "objeto_tamanho",
        IdentidadeCanonica::PorGrafia("json_objeto_tamanho"),
    ),
    exportar(
        JSON,
        "objeto_chaves",
        IdentidadeCanonica::PorGrafia("json_objeto_chaves"),
    ),
    exportar(JSON, "emitir", IdentidadeCanonica::PorGrafia("emitir_json")),
    exportar(
        JSON,
        "emitir_plano_bombom",
        IdentidadeCanonica::PorGrafia("emitir_json_plano_bombom"),
    ),
    exportar(
        JSON,
        "ler_plano_bombom",
        IdentidadeCanonica::PorGrafia("ler_json_plano_bombom"),
    ),
    exportar(
        JSON,
        "ler_resultado",
        IdentidadeCanonica::Falivel(OperacaoFalivel::InterpretarJson),
    ),
    // ----- módulo `lista` -----
    exportar(LISTA, "criar", IdentidadeCanonica::PorGrafia("lista_criar")),
    exportar(LISTA, "obter", IdentidadeCanonica::PorGrafia("lista_obter")),
    exportar(
        LISTA,
        "definir",
        IdentidadeCanonica::PorGrafia("lista_definir"),
    ),
    exportar(
        LISTA,
        "inserir",
        IdentidadeCanonica::PorGrafia("lista_inserir"),
    ),
    exportar(
        LISTA,
        "anexar",
        IdentidadeCanonica::PorGrafia("lista_anexar"),
    ),
    exportar(
        LISTA,
        "tamanho",
        IdentidadeCanonica::PorGrafia("lista_tamanho"),
    ),
    exportar(
        LISTA,
        "tirar_ultimo",
        IdentidadeCanonica::PorGrafia("lista_tirar_ultimo"),
    ),
    exportar(
        LISTA,
        "bombom_criar",
        IdentidadeCanonica::PorGrafia("lista_bombom_criar"),
    ),
    exportar(
        LISTA,
        "bombom_obter",
        IdentidadeCanonica::PorGrafia("lista_bombom_obter"),
    ),
    exportar(
        LISTA,
        "bombom_definir",
        IdentidadeCanonica::PorGrafia("lista_bombom_definir"),
    ),
    exportar(
        LISTA,
        "bombom_inserir",
        IdentidadeCanonica::PorGrafia("lista_bombom_inserir"),
    ),
    exportar(
        LISTA,
        "bombom_anexar",
        IdentidadeCanonica::PorGrafia("lista_bombom_anexar"),
    ),
    exportar(
        LISTA,
        "bombom_tamanho",
        IdentidadeCanonica::PorGrafia("lista_bombom_tamanho"),
    ),
    exportar(
        LISTA,
        "bombom_tirar_ultimo",
        IdentidadeCanonica::PorGrafia("lista_bombom_tirar_ultimo"),
    ),
    exportar(
        LISTA,
        "verso_criar",
        IdentidadeCanonica::PorGrafia("lista_verso_criar"),
    ),
    exportar(
        LISTA,
        "verso_obter",
        IdentidadeCanonica::PorGrafia("lista_verso_obter"),
    ),
    exportar(
        LISTA,
        "verso_definir",
        IdentidadeCanonica::PorGrafia("lista_verso_definir"),
    ),
    exportar(
        LISTA,
        "verso_inserir",
        IdentidadeCanonica::PorGrafia("lista_verso_inserir"),
    ),
    exportar(
        LISTA,
        "verso_anexar",
        IdentidadeCanonica::PorGrafia("lista_verso_anexar"),
    ),
    exportar(
        LISTA,
        "verso_tamanho",
        IdentidadeCanonica::PorGrafia("lista_verso_tamanho"),
    ),
    exportar(
        LISTA,
        "verso_tirar_ultimo",
        IdentidadeCanonica::PorGrafia("lista_verso_tirar_ultimo"),
    ),
    // ----- módulo `mapa` -----
    // #532: `criar` é a criação genérica de mapa, gêmea estrutural de
    // `lista.criar`. Ela era a última grafia builtin chamável sem import, e o
    // par `(mapa, criar)` não é escolha lexical nova: é a mesma tradução
    // `<familia>_<membro> -> familia.membro` que a #505 já aplicou aos outros
    // 29 membros deste módulo e ao `lista_criar` de mesma natureza.
    exportar(MAPA, "criar", IdentidadeCanonica::PorGrafia("mapa_criar")),
    exportar(
        MAPA,
        "definir",
        IdentidadeCanonica::PorGrafia("mapa_definir"),
    ),
    exportar(MAPA, "obter", IdentidadeCanonica::PorGrafia("mapa_obter")),
    exportar(
        MAPA,
        "remover",
        IdentidadeCanonica::PorGrafia("mapa_remover"),
    ),
    exportar(
        MAPA,
        "tamanho",
        IdentidadeCanonica::PorGrafia("mapa_tamanho"),
    ),
    exportar(MAPA, "tem", IdentidadeCanonica::PorGrafia("mapa_tem")),
    exportar(
        MAPA,
        "bombom_bombom_criar",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_criar"),
    ),
    exportar(
        MAPA,
        "bombom_bombom_definir",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_definir"),
    ),
    exportar(
        MAPA,
        "bombom_bombom_obter",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_obter"),
    ),
    exportar(
        MAPA,
        "bombom_bombom_remover",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_remover"),
    ),
    exportar(
        MAPA,
        "bombom_bombom_tamanho",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_tamanho"),
    ),
    exportar(
        MAPA,
        "bombom_bombom_tem",
        IdentidadeCanonica::PorGrafia("mapa_bombom_bombom_tem"),
    ),
    exportar(
        MAPA,
        "bombom_verso_criar",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_criar"),
    ),
    exportar(
        MAPA,
        "bombom_verso_definir",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_definir"),
    ),
    exportar(
        MAPA,
        "bombom_verso_obter",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_obter"),
    ),
    exportar(
        MAPA,
        "bombom_verso_remover",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_remover"),
    ),
    exportar(
        MAPA,
        "bombom_verso_tamanho",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_tamanho"),
    ),
    exportar(
        MAPA,
        "bombom_verso_tem",
        IdentidadeCanonica::PorGrafia("mapa_bombom_verso_tem"),
    ),
    exportar(
        MAPA,
        "verso_bombom_criar",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_criar"),
    ),
    exportar(
        MAPA,
        "verso_bombom_definir",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_definir"),
    ),
    exportar(
        MAPA,
        "verso_bombom_obter",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_obter"),
    ),
    exportar(
        MAPA,
        "verso_bombom_remover",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_remover"),
    ),
    exportar(
        MAPA,
        "verso_bombom_tamanho",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_tamanho"),
    ),
    exportar(
        MAPA,
        "verso_bombom_tem",
        IdentidadeCanonica::PorGrafia("mapa_verso_bombom_tem"),
    ),
    exportar(
        MAPA,
        "verso_verso_criar",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_criar"),
    ),
    exportar(
        MAPA,
        "verso_verso_definir",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_definir"),
    ),
    exportar(
        MAPA,
        "verso_verso_obter",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_obter"),
    ),
    exportar(
        MAPA,
        "verso_verso_remover",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_remover"),
    ),
    exportar(
        MAPA,
        "verso_verso_tamanho",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_tamanho"),
    ),
    exportar(
        MAPA,
        "verso_verso_tem",
        IdentidadeCanonica::PorGrafia("mapa_verso_verso_tem"),
    ),
    // ----- módulo `memoria` -----
    exportar_homonima(MEMORIA, IdentidadeCanonica::PorGrafia("alocar")),
    exportar_homonima(MEMORIA, IdentidadeCanonica::PorGrafia("liberar")),
    // ----- módulo `processo` -----
    exportar(
        PROCESSO,
        "executar",
        IdentidadeCanonica::PorGrafia("executar_processo"),
    ),
    exportar(
        PROCESSO,
        "executar_resultado",
        IdentidadeCanonica::Falivel(OperacaoFalivel::ExecutarProcesso),
    ),
    exportar(
        PROCESSO,
        "executar_estruturado",
        IdentidadeCanonica::Falivel(OperacaoFalivel::ExecutarProcessoEstruturado),
    ),
    exportar_homonima(
        PROCESSO,
        IdentidadeCanonica::PorGrafia("executar_com_entrada"),
    ),
    exportar_homonima(PROCESSO, IdentidadeCanonica::PorGrafia("capturar_stdout")),
    exportar_homonima(PROCESSO, IdentidadeCanonica::PorGrafia("capturar_stderr")),
    exportar_homonima(PROCESSO, IdentidadeCanonica::PorGrafia("pipeline_minimo")),
    exportar(
        PROCESSO,
        "codigo",
        IdentidadeCanonica::PorGrafia("processo_codigo"),
    ),
    exportar(
        PROCESSO,
        "saida",
        IdentidadeCanonica::PorGrafia("processo_saida"),
    ),
    exportar(
        PROCESSO,
        "erro",
        IdentidadeCanonica::PorGrafia("processo_erro"),
    ),
    exportar_homonima(PROCESSO, IdentidadeCanonica::PorGrafia("sair")),
    // ----- módulo `tempo` -----
    exportar(TEMPO, "unix", IdentidadeCanonica::PorGrafia("tempo_unix")),
    exportar(
        TEMPO,
        "formatar_unix",
        IdentidadeCanonica::PorGrafia("formatar_tempo_unix"),
    ),
    exportar_homonima(TEMPO, IdentidadeCanonica::PorGrafia("dormir")),
    // ----- módulo `texto` -----
    exportar(
        TEXTO,
        "aparar",
        IdentidadeCanonica::PorGrafia("aparar_verso"),
    ),
    exportar(
        TEXTO,
        "buscar",
        IdentidadeCanonica::PorGrafia("buscar_verso"),
    ),
    exportar_homonima(TEXTO, IdentidadeCanonica::PorGrafia("comeca_com")),
    exportar_homonima(TEXTO, IdentidadeCanonica::PorGrafia("termina_com")),
    exportar(
        TEXTO,
        "contem",
        IdentidadeCanonica::PorGrafia("contem_verso"),
    ),
    exportar(
        TEXTO,
        "dividir_contar",
        IdentidadeCanonica::PorGrafia("dividir_verso_contar"),
    ),
    exportar(
        TEXTO,
        "dividir_em",
        IdentidadeCanonica::PorGrafia("dividir_verso_em"),
    ),
    exportar(
        TEXTO,
        "fatiar",
        IdentidadeCanonica::PorGrafia("fatiar_verso"),
    ),
    exportar(
        TEXTO,
        "formatar",
        IdentidadeCanonica::PorGrafia("formatar_verso"),
    ),
    exportar(TEXTO, "igual", IdentidadeCanonica::PorGrafia("igual_verso")),
    exportar(
        TEXTO,
        "indice",
        IdentidadeCanonica::PorGrafia("indice_verso"),
    ),
    exportar(
        TEXTO,
        "indice_em",
        IdentidadeCanonica::PorGrafia("indice_verso_em"),
    ),
    exportar(
        TEXTO,
        "juntar",
        IdentidadeCanonica::PorGrafia("juntar_verso"),
    ),
    exportar(
        TEXTO,
        "juntar_com",
        IdentidadeCanonica::PorGrafia("juntar_verso_com"),
    ),
    exportar(
        TEXTO,
        "maiusculo",
        IdentidadeCanonica::PorGrafia("maiusculo_verso"),
    ),
    exportar(
        TEXTO,
        "minusculo",
        IdentidadeCanonica::PorGrafia("minusculo_verso"),
    ),
    exportar(
        TEXTO,
        "nao_vazio",
        IdentidadeCanonica::PorGrafia("nao_vazio_verso"),
    ),
    exportar(
        TEXTO,
        "substituir",
        IdentidadeCanonica::PorGrafia("substituir_verso"),
    ),
    exportar(
        TEXTO,
        "tamanho",
        IdentidadeCanonica::PorGrafia("tamanho_verso"),
    ),
    exportar(TEXTO, "vazio", IdentidadeCanonica::PorGrafia("vazio_verso")),
    exportar_homonima(TEXTO, IdentidadeCanonica::PorGrafia("bombom_para_verso")),
    exportar_homonima(TEXTO, IdentidadeCanonica::PorGrafia("verso_para_bombom")),
    exportar_homonima(
        TEXTO,
        IdentidadeCanonica::Falivel(OperacaoFalivel::ConverterVersoParaBombom),
    ),
];

/// A família é built-in importável?
pub fn familia_conhecida(familia: &str) -> bool {
    FAMILIAS.contains(&familia)
}

/// #532 — autoridade ÚNICA da precedência `REAL_MODULE_X > BUILTIN_FAMILY_X`.
///
/// A pergunta "quem governa este nome de import" tinha duas respostas: a forma
/// seletiva cedia a vez ao módulo real e a forma inteira não, o que fazia
/// `trazer texto;` ignorar um `texto.pink` que `trazer texto.X;` enxergava. A
/// precedência é da IDENTIDADE do nome, não da forma sintática que o escreveu.
///
/// ```text
/// FAMILY_GOVERNS(x) = KNOWN_FAMILY(x) && !REAL_MODULE_EXISTS(x)
/// ```
///
/// `modulo_real_existe` é um veredito observado — disco na CLI, grafo na
/// resolução —, não uma segunda política: esta autoridade não procura arquivo e
/// não sabe procurar. G-517-1 continua valendo por construção: ausência de
/// `<familia>.pink` é o caso comum de uma família legítima, e não um módulo
/// faltando.
pub fn familia_governa(familia: &str, modulo_real_existe: bool) -> bool {
    familia_conhecida(familia) && !modulo_real_existe
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
/// Depois da #505 não existe módulo importável sem membro — o gate
/// `os_modulos_sao_exatamente_os_quinze_aceitos` prova a igualdade entre o
/// conjunto declarado e o conjunto que exporta —, então o ramo «não exporta
/// membros nesta fase» deixou de ser alcançável por fonte e saiu junto com a
/// fase que o justificava.
pub fn membro_inexistente(familia: &str, membro: &str) -> String {
    let membros = membros_da_familia(familia);
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

/// Módulos que exportam um membro com esta grafia, em ordem de declaração.
///
/// Grafia de membro não é única entre módulos — `acaso.criar` e
/// `arquivo.criar` existem ao mesmo tempo —, então a resposta é uma lista.
pub fn modulos_que_exportam(membro: &str) -> Vec<&'static str> {
    let mut modulos: Vec<&'static str> = Vec::new();
    for exportacao in EXPORTACOES {
        if exportacao.membro() == membro && !modulos.contains(&exportacao.familia) {
            modulos.push(exportacao.familia);
        }
    }
    modulos
}

/// O par `(módulo, membro)` que expõe uma identidade endereçada por grafia
/// canônica.
///
/// Existe para o diagnóstico: quem escreveu a grafia canônica precisa saber
/// qual import a torna chamável. `None` significa que a grafia não é
/// endereçada por nenhum membro — o que, depois da #505, é um defeito de
/// registro, não um estado normal.
pub fn par_da_grafia_canonica(grafia: &str) -> Option<(&'static str, &'static str)> {
    EXPORTACOES
        .iter()
        .find(|exportacao| exportacao.identidade.nome_publico() == grafia)
        .map(|exportacao| (exportacao.familia, exportacao.membro()))
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
