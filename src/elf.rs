//! Leitor mínimo de ELF64 little-endian.
//!
//! O workspace é deliberadamente sem dependências externas (ver `Cargo.toml`),
//! então não existe um parser de objeto já presente para reutilizar. Em vez de
//! ler a saída textual de `readelf`/`nm` — que muda entre versões e locales, e
//! que teria de ser reparseada a cada build — este módulo lê os campos do ELF
//! diretamente dos bytes.
//!
//! O escopo é o que a política de `sussurro` precisa provar sobre o objeto
//! realmente produzido: **quais seções existem** e **quais símbolos são
//! definidos**, com ligação, visibilidade e tipo. Nada além disso é
//! interpretado: relocações, notas, DWARF e conteúdo de seção são ignorados.

// @pinker-nav:start build.elf.leitor
// @pinker-nav:domain build
// @pinker-nav:layer elf
// @pinker-nav:summary Leitor mínimo de ELF64 little-endian sem dependência externa: valida o magic `\x7fELF`, a classe 64 bits, a ordem little-endian e a consistência de `e_shentsize`/`e_shoff`, resolve `e_shnum`/`e_shstrndx` inclusive nas formas estendidas (`shnum == 0` lê `sh_size` da seção 0 e `shstrndx == SHN_XINDEX` lê `sh_link`), coleta os nomes das seções pela `.shstrtab` e percorre toda seção `SHT_SYMTAB` extraindo nome, ligação (`st_info >> 4`), tipo (`st_info & 0xf`), visibilidade (`st_other & 0x3`), índice de seção e tamanho de cada símbolo. Toda leitura é limitada por índice conferido contra o tamanho do buffer, de modo que um arquivo truncado ou malformado devolve `Err` com detalhe legível em vez de pânico; nenhum conteúdo de seção, relocação ou informação de depuração é interpretado.

/// Índice de seção especial: símbolo apenas referenciado, não definido aqui.
pub const SHN_UNDEF: u16 = 0;
/// Índice de seção especial: valor absoluto, não pertence a nenhuma seção.
pub const SHN_ABS: u16 = 0xfff1;
/// Índice de seção especial: símbolo comum (alocado pelo linker).
pub const SHN_COMMON: u16 = 0xfff2;
/// Marcador de que o índice real está em outra tabela.
const SHN_XINDEX: u16 = 0xffff;

const SHT_SYMTAB: u32 = 2;

/// Um símbolo lido da tabela de símbolos do objeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfSymbol {
    pub name: String,
    /// `STB_LOCAL` = 0, `STB_GLOBAL` = 1, `STB_WEAK` = 2.
    pub bind: u8,
    /// `STT_NOTYPE` = 0, `STT_OBJECT` = 1, `STT_FUNC` = 2, `STT_SECTION` = 3.
    pub symbol_type: u8,
    /// `STV_DEFAULT` = 0, `STV_INTERNAL` = 1, `STV_HIDDEN` = 2, `STV_PROTECTED` = 3.
    pub visibility: u8,
    pub section_index: u16,
    pub size: u64,
}

impl ElfSymbol {
    /// O símbolo é **definido** neste objeto, e não apenas referenciado?
    pub fn is_defined(&self) -> bool {
        self.section_index != SHN_UNDEF
    }
}

/// A superfície de um objeto ELF relevante para a política de `sussurro`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfObject {
    pub sections: Vec<String>,
    pub symbols: Vec<ElfSymbol>,
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let end = at.checked_add(2).ok_or_else(|| overflow(at))?;
    let slice = bytes.get(at..end).ok_or_else(|| truncated(at, 2))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let end = at.checked_add(4).ok_or_else(|| overflow(at))?;
    let slice = bytes.get(at..end).ok_or_else(|| truncated(at, 4))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, String> {
    let end = at.checked_add(8).ok_or_else(|| overflow(at))?;
    let slice = bytes.get(at..end).ok_or_else(|| truncated(at, 8))?;
    let mut buffer = [0u8; 8];
    buffer.copy_from_slice(slice);
    Ok(u64::from_le_bytes(buffer))
}

fn overflow(at: usize) -> String {
    format!("deslocamento {at} estoura o espaço de endereçamento do ELF")
}

fn truncated(at: usize, width: usize) -> String {
    format!("ELF truncado: {width} byte(s) em {at} estão fora do arquivo")
}

/// Converte um `u64` de deslocamento do ELF em índice utilizável.
fn as_index(value: u64, what: &str) -> Result<usize, String> {
    usize::try_from(value)
        .map_err(|_| format!("{what} ({value}) não cabe no endereçamento do host"))
}

/// Lê uma string terminada em NUL de uma tabela de strings.
fn read_str(bytes: &[u8], table: (usize, usize), offset: u32) -> Result<String, String> {
    let (start, size) = table;
    let offset = offset as usize;
    if offset >= size {
        return Err(format!(
            "deslocamento {offset} fora da tabela de strings de {size} byte(s)"
        ));
    }
    let begin = start + offset;
    let table_end = start + size;
    let slice = bytes
        .get(begin..table_end)
        .ok_or_else(|| truncated(begin, table_end - begin))?;
    let end = slice
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(slice.len());
    Ok(String::from_utf8_lossy(&slice[..end]).into_owned())
}

/// Lê seções e símbolos de um objeto ELF64 little-endian.
///
/// Devolve `Err` com detalhe legível para qualquer arquivo que não seja
/// exatamente isso: magic errado, classe 32 bits, big-endian, cabeçalho de
/// seção com tamanho inesperado ou qualquer leitura fora dos limites.
pub fn parse(bytes: &[u8]) -> Result<ElfObject, String> {
    if bytes.len() < 64 {
        return Err(format!(
            "arquivo com {} byte(s) é menor que um cabeçalho ELF64",
            bytes.len()
        ));
    }
    if &bytes[0..4] != b"\x7fELF" {
        return Err("arquivo não começa com o magic ELF".to_string());
    }
    if bytes[4] != 2 {
        return Err(format!("classe ELF {} não é ELF64", bytes[4]));
    }
    if bytes[5] != 1 {
        return Err(format!("ordem de bytes {} não é little-endian", bytes[5]));
    }

    let section_header_offset = as_index(read_u64(bytes, 0x28)?, "e_shoff")?;
    let section_header_size = read_u16(bytes, 0x3a)? as usize;
    let mut section_count = read_u16(bytes, 0x3c)? as usize;
    let mut string_table_index = read_u16(bytes, 0x3e)? as usize;

    if section_header_offset == 0 {
        // Sem tabela de seções não há nem seção nem símbolo a inspecionar.
        return Ok(ElfObject {
            sections: Vec::new(),
            symbols: Vec::new(),
        });
    }
    if section_header_size != 64 {
        return Err(format!(
            "cabeçalho de seção com {section_header_size} byte(s); ELF64 exige 64"
        ));
    }

    // Formas estendidas: com mais de 0xff00 seções, `e_shnum` e `e_shstrndx`
    // moram na seção 0.
    let first_header = section_header_offset;
    if section_count == 0 {
        section_count = as_index(read_u64(bytes, first_header + 0x20)?, "sh_size da seção 0")?;
    }
    if string_table_index == SHN_XINDEX as usize {
        string_table_index = read_u32(bytes, first_header + 0x28)? as usize;
    }

    let header_at = |index: usize| -> Result<usize, String> {
        index
            .checked_mul(section_header_size)
            .and_then(|scaled| scaled.checked_add(section_header_offset))
            .ok_or_else(|| format!("índice de seção {index} estoura o deslocamento"))
    };

    // Passo 1: deslocamento e tamanho brutos de cada seção, mais o nome cru.
    let mut raw: Vec<(u32, u32, usize, usize, u32, u64)> = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let at = header_at(index)?;
        let name_offset = read_u32(bytes, at)?;
        let section_type = read_u32(bytes, at + 0x04)?;
        let offset = as_index(read_u64(bytes, at + 0x18)?, "sh_offset")?;
        let size = as_index(read_u64(bytes, at + 0x20)?, "sh_size")?;
        let link = read_u32(bytes, at + 0x28)?;
        let entry_size = read_u64(bytes, at + 0x38)?;
        raw.push((name_offset, section_type, offset, size, link, entry_size));
    }

    let strings = |index: usize| -> Result<(usize, usize), String> {
        let (_, _, offset, size, _, _) = *raw
            .get(index)
            .ok_or_else(|| format!("tabela de strings {index} não existe"))?;
        Ok((offset, size))
    };

    let name_table = strings(string_table_index)?;
    let mut sections = Vec::with_capacity(section_count);
    for (name_offset, _, _, _, _, _) in &raw {
        sections.push(read_str(bytes, name_table, *name_offset)?);
    }

    // Passo 2: toda `SHT_SYMTAB`. Objetos do `as` têm uma; binários linkados
    // podem ter `.symtab` e, separadamente, `.dynsym` (que não é `SHT_SYMTAB`).
    let mut symbols = Vec::new();
    for (_, section_type, offset, size, link, entry_size) in &raw {
        if *section_type != SHT_SYMTAB {
            continue;
        }
        if *entry_size != 24 {
            return Err(format!(
                "entrada de símbolo com {entry_size} byte(s); ELF64 exige 24"
            ));
        }
        let symbol_strings = strings(*link as usize)?;
        let count = size / 24;
        for index in 0..count {
            let at = offset + index * 24;
            let name_offset = read_u32(bytes, at)?;
            let info = *bytes.get(at + 4).ok_or_else(|| truncated(at + 4, 1))?;
            let other = *bytes.get(at + 5).ok_or_else(|| truncated(at + 5, 1))?;
            let section_index = read_u16(bytes, at + 6)?;
            let symbol_size = read_u64(bytes, at + 16)?;
            symbols.push(ElfSymbol {
                name: read_str(bytes, symbol_strings, name_offset)?,
                bind: info >> 4,
                symbol_type: info & 0x0f,
                visibility: other & 0x03,
                section_index,
                size: symbol_size,
            });
        }
    }

    Ok(ElfObject { sections, symbols })
}
// @pinker-nav:end build.elf.leitor
