# Backlog futuro (visão, não compromisso imediato)

Este documento organiza possibilidades futuras de evolução do Pinker v0 em camadas de dependência.

> Status: backlog estruturado para referência técnica; não é roadmap rígido de curto prazo.

## Camada 0 — Sistema de tipos (bloqueante para tudo)
- Tipos inteiros com tamanho: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` — hoje só existe `bombom` (`u64`). Kernel precisa de `u8` para bytes, `u16` para portas, `u32` para registradores etc.
- Tipo ponteiro (`seta`): `seta<T>` ou `seta bombom` — representar endereços de memória.
- Structs (`ninho`): dados compostos com layout de memória definido.
- Enums (`leque`): variantes com ou sem dados associados.
- Arrays de tamanho fixo: `[bombom; 256]`.
- Slices / arrays dinâmicos: referência a faixa de memória contígua.
- Tipo float (`bolha`): não crítico para kernel, mas útil para completude de médio nível.
- Tipo char/byte (`letra`/`migalha`): manipulação de texto e dados brutos.
- Strings (`verso`): pelo menos como slice de bytes.
- Tuples (`par`): retornos múltiplos e agrupamento leve.
- Type alias (`apelido`): ex. `apelido Endereco = u64`.
- Void pointer / ponteiro genérico: equivalente a `void*` em C.

## Camada 1 — Operações de memória (bloqueante para kernel)
- Dereferência de ponteiro: ler/escrever pelo endereço (`*seta`).
- Aritmética de ponteiros: `seta + offset`, percorrer memória/arrays.
- Cast entre tipos (`virar`): converter `u64` para `seta`, inteiros entre tamanhos.
- Acesso a campos de struct por offset: `registro.campo` -> base + offset.
- Acesso a array por índice: `vetor[i]` -> base + (`i * tamanho`).
- Volatile read/write (`frágil`): leitura/escrita que compilador não remove.
- Alocação manual (`reserva`/`soltar`) ou chamada explícita a alocador.
- `sizeof` (`peso`): tamanho de tipo em bytes em tempo de compilação.
- Align/alinhamento: controle de alinhamento de structs e campos.

## Camada 2 — Controle de fluxo e expressividade (necessário para self-hosting e completude)
- `for` / iteração (`passeio`).
- `match` / pattern matching (`encaixe`).
- Operadores lógicos `&&` e `||` com short-circuit.
- Operadores bitwise: `&`, `|`, `^`, `<<`, `>>`.
- Operadores compostos: `+=`, `-=`, `<<=`, `>>=`, `&=`, `|=`.
- Loop infinito (`roda`) com bloco explícito.
- Expressões como valor (estilo Rust), se fizer sentido.

## Camada 3 — Sistema de módulos e organização (bloqueante para self-hosting)
- Módulos (`canto`) para separar código em arquivos.
- Imports (`trazer`) de símbolos entre módulos.
- Visibilidade (`aberto`/`segredo`).
- Compilação separada + linkagem.
- Namespaces / resolução qualificada (`modulo.funcao()` ou `modulo::funcao()`).

## Camada 4 — Polimorfismo e abstração (necessário para self-hosting elegante)
- Traits/interfaces (`molde`).
- Impl blocks (`vestir`).
- Generics (`qualquer`) em funções/tipos.
- Ponteiros de função / closures.

## Camada 5 — Backend de código nativo (bloqueante para tudo executável)
- Geração de código para x86_64.
- Emissão de binário ELF (ou flat binary).
- Register allocation.
- Calling convention / ABI.
- Linker para combinar objetos compilados separadamente.
- Otimizações básicas: constant folding, dead code elimination, inlining.
- Segundo target opcional (RISC-V ou AArch64).

## Camada 6 — Capacidades de sistemas (bloqueante para kernel)
- Inline assembly (`sussurro`).
- Atributos de função (`#[naked]`, `#[no_mangle]`, `#[interrupt]`).
- No-std / freestanding.
- Linker script support.
- Constantes em tempo de compilação (const eval).
- Static mutável (`raiz mut`).
- Union / reinterpret cast.
- Packed structs.

## Camada 7 — Biblioteca padrão mínima (necessário para self-hosting)
- I/O básico (`falar`/`ouvir`).
- Manipulação de arquivos (`abrir`/`fechar`/`escrever`).
- Strings com operações.
- Coleções básicas (`Vec`, `HashMap`).
- Formatação de texto.
- Alocador de heap.
- Tratamento de erros (`amparo`/`tropeco`), equivalente a `Result`/`Option`.

## Camada 8 — Infraestrutura de build (necessário para self-hosting)
- Sistema de build próprio ou integrado para múltiplos `.pink`.
- Preprocessador ou macro system (opcional).
- Testes integrados do compilador sobre o próprio compilador.

## Resumo de distância
- Linguagem de uso geral -> Camadas 0 + 2 + 3 -> ~33 itens.
- Programação de sistemas -> + 1 + 5 + 6 -> ~52 itens.
- Criar um kernel -> + 6 completa -> ~52 itens.
- Self-hosting (compilar a si mesma) -> + 3 + 4 + 7 + 8 -> ~62 itens (todos).

## 5 itens mais críticos para desbloquear progresso
1. Backend x86_64 (#38) — sem código nativo, nada roda de verdade.
2. Ponteiros (#2, #13, #14) — sem ponteiros, nada de sistemas.
3. Structs (#3) — sem dados compostos úteis, pouco avança.
4. Operadores bitwise (#25) — sem manipulação de bits, sem hardware.
5. Inline assembly (#45) — sem falar com CPU, nada de kernel.

## Observações de maturidade
- A nomenclatura sugerida (ex.: `seta`, `ninho`, `leque`, `passeio`, `sussurro`) segue o estilo do `docs/vocabulario.md` como referência auxiliar.
- Este backlog é deliberadamente separado das fases curtas atuais para evitar inflar escopo do Pinker v0.
