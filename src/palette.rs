// ─────────────────────────────────────────────────────────────────────
//  Pinker — Paleta de cores para terminal e editores de texto
// ─────────────────────────────────────────────────────────────────────
//
//  Implementação 100 % sem dependências externas. Usa sequências ANSI
//  truecolor (24-bit: ESC[38;2;R;G;Bm) para colorir a saída textual
//  do compilador.  Respeita a variável de ambiente `NO_COLOR`
//  (https://no-color.org/) e a flag `--no-color`.

// ── Cor RGB ──────────────────────────────────────────────────────────

/// Representação de uma cor em RGB de 24 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Cria uma cor a partir de um literal hexadecimal `0xRRGGBB`.
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    /// Gera a sequência ANSI de foreground truecolor.
    pub fn fg_ansi(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Gera a sequência ANSI de background truecolor.
    pub fn bg_ansi(self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Retorna a representação hexadecimal `#RRGGBB`.
    pub fn to_hex_string(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

// ── Constantes da paleta ─────────────────────────────────────────────

/// Fundo principal do editor.
pub const FUNDO_PRINCIPAL: Rgb = Rgb::from_hex(0x0D0B14);

/// Fundo secundário / barras laterais e painéis.
pub const FUNDO_SECUNDARIO: Rgb = Rgb::from_hex(0x151120);

/// Texto normal (código, variáveis locais, pontuação genérica).
pub const TEXTO_NORMAL: Rgb = Rgb::from_hex(0xF4EFFF);

/// Texto suave — comentários e elementos de menor destaque.
pub const TEXTO_SUAVE: Rgb = Rgb::from_hex(0x9B8FB3);

/// Keyword principal da Pinker (rosa vivo).
pub const KEYWORD: Rgb = Rgb::from_hex(0xFF4FCB);

/// Tipos, nomes especiais, anotações.
pub const TIPO: Rgb = Rgb::from_hex(0xC792FF);

/// Literais string.
pub const STRING: Rgb = Rgb::from_hex(0xFFB3E6);

/// Literais numéricos.
pub const NUMERO: Rgb = Rgb::from_hex(0xFFC857);

/// Nomes de funções e chamadas.
pub const FUNCAO: Rgb = Rgb::from_hex(0x7EE7FF);

/// Operadores e delimitadores semânticos.
pub const OPERADOR: Rgb = Rgb::from_hex(0xE7D7FF);

/// Indicação de erro.
pub const ERRO: Rgb = Rgb::from_hex(0xFF5D73);

/// Indicação de aviso.
pub const AVISO: Rgb = Rgb::from_hex(0xFFB86B);

/// Indicação de sucesso.
pub const SUCESSO: Rgb = Rgb::from_hex(0x72F1B8);

/// Cor do cursor.
pub const CURSOR: Rgb = Rgb::from_hex(0xFF4FCB);

/// Cor de fundo da seleção.
pub const SELECAO: Rgb = Rgb::from_hex(0x2A1F3A);

// ── Tema estruturado ─────────────────────────────────────────────────

/// Agrupamento semântico de todas as cores da paleta.
#[derive(Debug, Clone, Copy)]
pub struct Tema {
    pub fundo_principal: Rgb,
    pub fundo_secundario: Rgb,
    pub texto_normal: Rgb,
    pub texto_suave: Rgb,
    pub keyword: Rgb,
    pub tipo: Rgb,
    pub string: Rgb,
    pub numero: Rgb,
    pub funcao: Rgb,
    pub operador: Rgb,
    pub erro: Rgb,
    pub aviso: Rgb,
    pub sucesso: Rgb,
    pub cursor: Rgb,
    pub selecao: Rgb,
}

/// Tema padrão da Pinker.
pub const TEMA_PINKER: Tema = Tema {
    fundo_principal: FUNDO_PRINCIPAL,
    fundo_secundario: FUNDO_SECUNDARIO,
    texto_normal: TEXTO_NORMAL,
    texto_suave: TEXTO_SUAVE,
    keyword: KEYWORD,
    tipo: TIPO,
    string: STRING,
    numero: NUMERO,
    funcao: FUNCAO,
    operador: OPERADOR,
    erro: ERRO,
    aviso: AVISO,
    sucesso: SUCESSO,
    cursor: CURSOR,
    selecao: SELECAO,
};

// ── Reset ANSI ───────────────────────────────────────────────────────

pub const RESET: &str = "\x1b[0m";

// ── Helpers para estilização ─────────────────────────────────────────

/// Envolve `texto` com a cor de foreground indicada e reset ao final.
pub fn colorir(cor: Rgb, texto: &str) -> String {
    format!("{}{}{}", cor.fg_ansi(), texto, RESET)
}

/// Envolve `texto` com cor de foreground + background e reset ao final.
pub fn colorir_com_fundo(fg: Rgb, bg: Rgb, texto: &str) -> String {
    format!("{}{}{}{}", fg.fg_ansi(), bg.bg_ansi(), texto, RESET)
}

/// Aplica negrito ANSI antes da cor.
pub fn negrito(cor: Rgb, texto: &str) -> String {
    format!("\x1b[1m{}{}{}", cor.fg_ansi(), texto, RESET)
}

/// Aplica itálico ANSI antes da cor.
pub fn italico(cor: Rgb, texto: &str) -> String {
    format!("\x1b[3m{}{}{}", cor.fg_ansi(), texto, RESET)
}

/// Aplica sublinhado ANSI antes da cor.
pub fn sublinhado(cor: Rgb, texto: &str) -> String {
    format!("\x1b[4m{}{}{}", cor.fg_ansi(), texto, RESET)
}

// ── Detecção de suporte a cor ────────────────────────────────────────

/// Retorna `true` se a saída colorida deve ser suprimida.
///
/// Respeita a convenção `NO_COLOR` (qualquer valor ativa a supressão)
/// e, opcionalmente, pode ser forçada com `--no-color` na CLI.
pub fn sem_cor() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// Versão condicional de `colorir`: retorna texto puro se cores estiverem
/// desativadas.
pub fn colorir_se(cor: Rgb, texto: &str) -> String {
    if sem_cor() {
        texto.to_string()
    } else {
        colorir(cor, texto)
    }
}

/// Versão condicional de `negrito`.
pub fn negrito_se(cor: Rgb, texto: &str) -> String {
    if sem_cor() {
        texto.to_string()
    } else {
        negrito(cor, texto)
    }
}

// ── Exportação em formato editor ─────────────────────────────────────

/// Gera um resumo textual da paleta com os valores hexadecimais,
/// útil como referência para integrações com editores de texto.
pub fn resumo_paleta() -> String {
    let t = TEMA_PINKER;
    format!(
        "\
Pinker — Paleta de Cores
========================

Fundo principal       {fundo_principal}
Fundo secundário      {fundo_sec}
Texto normal          {texto_normal}
Texto suave           {texto_suave}
Keyword               {keyword}
Tipos                 {tipo}
Strings               {string}
Números               {numero}
Funções               {funcao}
Operadores            {operador}
Erro                  {erro}
Aviso                 {aviso}
Sucesso               {sucesso}
Cursor                {cursor}
Seleção               {selecao}",
        fundo_principal = t.fundo_principal.to_hex_string(),
        fundo_sec = t.fundo_secundario.to_hex_string(),
        texto_normal = t.texto_normal.to_hex_string(),
        texto_suave = t.texto_suave.to_hex_string(),
        keyword = t.keyword.to_hex_string(),
        tipo = t.tipo.to_hex_string(),
        string = t.string.to_hex_string(),
        numero = t.numero.to_hex_string(),
        funcao = t.funcao.to_hex_string(),
        operador = t.operador.to_hex_string(),
        erro = t.erro.to_hex_string(),
        aviso = t.aviso.to_hex_string(),
        sucesso = t.sucesso.to_hex_string(),
        cursor = t.cursor.to_hex_string(),
        selecao = t.selecao.to_hex_string(),
    )
}

// ── Testes ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_from_hex_correto() {
        let cor = Rgb::from_hex(0xFF4FCB);
        assert_eq!(cor.r, 0xFF);
        assert_eq!(cor.g, 0x4F);
        assert_eq!(cor.b, 0xCB);
    }

    #[test]
    fn to_hex_string_formato_correto() {
        assert_eq!(KEYWORD.to_hex_string(), "#FF4FCB");
        assert_eq!(FUNDO_PRINCIPAL.to_hex_string(), "#0D0B14");
        assert_eq!(SUCESSO.to_hex_string(), "#72F1B8");
    }

    #[test]
    fn fg_ansi_formato_correto() {
        let ansi = Rgb::from_hex(0xFF4FCB).fg_ansi();
        assert_eq!(ansi, "\x1b[38;2;255;79;203m");
    }

    #[test]
    fn bg_ansi_formato_correto() {
        let ansi = Rgb::from_hex(0x2A1F3A).bg_ansi();
        assert_eq!(ansi, "\x1b[48;2;42;31;58m");
    }

    #[test]
    fn colorir_inclui_reset() {
        let resultado = colorir(KEYWORD, "carinho");
        assert!(resultado.starts_with("\x1b[38;2;255;79;203m"));
        assert!(resultado.ends_with(RESET));
        assert!(resultado.contains("carinho"));
    }

    #[test]
    fn negrito_inclui_bold_e_reset() {
        let resultado = negrito(ERRO, "falha");
        assert!(resultado.starts_with("\x1b[1m"));
        assert!(resultado.ends_with(RESET));
        assert!(resultado.contains("falha"));
    }

    #[test]
    fn tema_pinker_cores_consistentes() {
        assert_eq!(TEMA_PINKER.keyword, KEYWORD);
        assert_eq!(TEMA_PINKER.cursor, CURSOR);
        assert_eq!(TEMA_PINKER.keyword, TEMA_PINKER.cursor); // ambos #FF4FCB
    }

    #[test]
    fn resumo_paleta_contem_todas_as_cores() {
        let resumo = resumo_paleta();
        assert!(resumo.contains("#0D0B14"));
        assert!(resumo.contains("#151120"));
        assert!(resumo.contains("#F4EFFF"));
        assert!(resumo.contains("#9B8FB3"));
        assert!(resumo.contains("#FF4FCB"));
        assert!(resumo.contains("#C792FF"));
        assert!(resumo.contains("#FFB3E6"));
        assert!(resumo.contains("#FFC857"));
        assert!(resumo.contains("#7EE7FF"));
        assert!(resumo.contains("#E7D7FF"));
        assert!(resumo.contains("#FF5D73"));
        assert!(resumo.contains("#FFB86B"));
        assert!(resumo.contains("#72F1B8"));
        assert!(resumo.contains("#2A1F3A"));
    }
}
