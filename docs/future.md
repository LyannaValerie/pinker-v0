# Backlog futuro (visão, não compromisso imediato)

> **Nota de precedência**: `docs/future.md` é inventário amplo de possibilidades e backlog de longo prazo.
> A ordem oficial ativa de implementação está em `docs/roadmap.md`.
> Este documento **não é roadmap**, não dita ordem ativa e não substitui `docs/roadmap.md`.
>
> **Distinção importante**: este arquivo é inventário técnico estruturado — itens com status de implementação, organizados por camadas de dependência.
> `docs/parallel.md` é diferente: é o documento visionário da Pinker (fantasia orientadora, identidade, sonhos); não contém itens técnicos com status, não é backlog e não compete com este arquivo.

> **Trilha ativa atual**: **Bloco 7 — Backend nativo real** (definido em `docs/roadmap.md`).
> O Bloco 6 — Memória operacional foi concluído (Fases 64–72). O Bloco 7 é a próxima trilha ativa.
> O **Bloco 8 — I/O e ecossistema útil** está definido como bloco futuro planejado, mas não ativo.
> Itens como terminal próprio, self-hosting, backend nativo completo, kernel real robusto, package manager e ecossistema soberano permanecem aqui como visão de longo prazo, sem competir com a trilha ativa.

Este documento organiza possibilidades futuras de evolução do Pinker v0 em camadas de dependência.
Cada item tem status: ✅ implementado, 🔶 parcial, ou sem marcação (não iniciado / ideia futura).

> **Status geral**: backlog estruturado para referência técnica; não é roadmap rígido de curto prazo.

---

## Camada 0 — Sistema de tipos (bloqueante para tudo)
- ~~Tipos inteiros com tamanho: `u8`–`u64`, `i8`–`i64`.~~ ✅ Implementado (Fases 43–44). Runtime signed ainda u64-only (bloqueado explicitamente por HF-3 até representação correta).
- ~~Tipo ponteiro (`seta`): `seta<T>`.~~ ✅ Implementado (Fase 48). Semântica de tipo presente; dereferência/runtime pendente.
- ~~Structs (`ninho`): dados compostos com layout de memória definido.~~ 🔶 Declaração + semântica + layout estático (Fase 47 + Fase 51). Acesso a campo em leitura (Fase 49). Escrita em campo, construtor literal e layout de memória operacional pendentes.
- Enums (`leque`): variantes com ou sem dados associados.
- ~~Arrays de tamanho fixo: `[bombom; 256]`.~~ 🔶 Tipo na AST/semântica (Fase 46). Indexação em leitura (Fase 49). Escrita por índice, bounds-check e runtime de memória pendentes.
- Slices / arrays dinâmicos: referência a faixa de memória contígua.
- Tipo float (`bolha`): não crítico para kernel, mas útil para completude de médio nível.
- Tipo char/byte (`letra`/`migalha`): manipulação de texto e dados brutos.
- Strings (`verso`): pelo menos como slice de bytes.
- Tuples (`par`): retornos múltiplos e agrupamento leve.
- ~~Type alias (`apelido`): ex. `apelido Endereco = u64`.~~ ✅ Implementado (Fase 45).
- Void pointer / ponteiro genérico: equivalente a `void*` em C.

---

## Camada 1 — Operações de memória (bloqueante para kernel)
- Dereferência de ponteiro: ler/escrever pelo endereço (`*seta`).
- Aritmética de ponteiros: `seta + offset`, percorrer memória/arrays.
- ~~Cast entre tipos (`virar`): converter inteiros entre tamanhos.~~ 🔶 Parcialmente implementado (Fase 50). Cast inteiro→inteiro (`bombom`, `u8/u16/u32/u64`, `i8/i16/i32/i64`) no frontend/semântica/IR. Lowering operacional em CFG/Machine/runtime e cast de/para ponteiros ainda pendentes.
- ~~Acesso a campos de struct por offset: `registro.campo`.~~ 🔶 Parcialmente implementado (Fase 49). Leitura semântica e representação na IR; escrita em LHS, lowering de memória e layout físico por offset pendentes.
- ~~Acesso a array por índice: `vetor[i]`.~~ 🔶 Parcialmente implementado (Fase 49). Leitura semântica e representação na IR; escrita por índice, bounds-check e runtime de memória pendentes.
- Volatile read/write (`frágil`): leitura/escrita que compilador não remove.
- Alocação manual (`reserva`/`soltar`) ou chamada explícita a alocador.
- ~~`sizeof` (`peso`): tamanho de tipo em bytes em tempo de compilação.~~ ✅ Implementado (Fase 51). Cálculo estático de layout para todos os tipos atuais; sem runtime novo.
- ~~Align/alinhamento: controle de alinhamento de tipos e structs.~~ ✅ Implementado (Fase 51). `alinhamento(tipo)` como consulta estática; sem ABI/layout físico final.

---

## Camada 2 — Controle de fluxo e expressividade (necessário para self-hosting e completude)
- `for` / iteração (`passeio`).
- `match` / pattern matching (`encaixe`).
- ~~Operadores lógicos `&&` e `||` com short-circuit.~~ ✅ Implementado (Fase 36).
- ~~Operadores bitwise: `&`, `|`, `^`, `<<`, `>>`.~~ ✅ Implementado (Fase 34).
- Operadores compostos: `+=`, `-=`, `<<=`, `>>=`, `&=`, `|=`.
- Loop infinito (`roda`) com bloco explícito.
- Expressões como valor (estilo Rust), se fizer sentido.

---

## Camada 3 — Sistema de módulos e organização (bloqueante para self-hosting)
- Módulos (`canto`) para separar código em arquivos.
- Imports (`trazer`) de símbolos entre módulos.
- Visibilidade (`aberto`/`segredo`).
- Compilação separada + linkagem.
- Namespaces / resolução qualificada (`modulo.funcao()` ou `modulo::funcao()`).

---

## Camada 4 — Polimorfismo e abstração (necessário para self-hosting elegante)
- Traits/interfaces (`molde`).
- Impl blocks (`vestir`).
- Generics (`qualquer`) em funções/tipos.
- Ponteiros de função / closures.

---

## Camada 5 — Backend de código nativo (bloqueante para tudo executável)
- Geração de código para x86_64.
- Emissão de binário ELF (ou flat binary).
- Register allocation.
- Calling convention / ABI.
- Linker para combinar objetos compilados separadamente.
- Otimizações básicas: constant folding, dead code elimination, inlining.
- Segundo target opcional (RISC-V ou AArch64).

---

## Camada 6 — Capacidades de sistemas (bloqueante para kernel)
- Inline assembly (`sussurro`).
- Atributos de função (`#[naked]`, `#[no_mangle]`, `#[interrupt]`).
- No-std / freestanding.
- Linker script support.
- Constantes em tempo de compilação (const eval).
- Static mutável (`raiz mut`).
- Union / reinterpret cast.
- Packed structs.

---

## Camada 7 — Biblioteca padrão mínima (necessário para self-hosting)
- I/O básico (`falar`/`ouvir`).
- Manipulação de arquivos (`abrir`/`fechar`/`escrever`).
- Strings com operações.
- Coleções básicas (`Vec`, `HashMap`).
- Formatação de texto.
- Alocador de heap.
- Tratamento de erros (`amparo`/`tropeco`), equivalente a `Result`/`Option`.

---

## Camada 8 — Infraestrutura de build (necessário para self-hosting)
- Sistema de build próprio ou integrado para múltiplos `.pink`.
- Preprocessador ou macro system (opcional).
- Testes integrados do compilador sobre o próprio compilador.

---

## Resumo de distância (aproximado)
- Linguagem de uso geral → Camadas 0 + 2 + 3 → ~33 itens (vários já realizados parcial ou totalmente).
- Programação de sistemas → + 1 + 5 + 6 → ~52 itens.
- Criar um kernel → + 6 completa → ~52 itens.
- Self-hosting (compilar a si mesma) → + 3 + 4 + 7 + 8 → ~62 itens (todos).

---

## Frentes prioritárias em aberto (backlog, sem ditar ordem ativa)

Os itens abaixo representam as maiores lacunas abertas em relação ao estado atual.
A ordem de execução está definida em `docs/roadmap.md`, não aqui.

1. **Backend x86_64** — sem código nativo, nada roda de verdade.
2. **Dereferência de ponteiro + aritmética** — `seta` existe no tipo, falta runtime e semântica operacional.
3. **Cast operacional (`virar`)** — inteiro→inteiro está no frontend/semântica/IR; lowering CFG/Machine/runtime e cast de/para ponteiros pendentes.
4. **Acesso a campo e indexação com escrita + runtime** — leitura semântica existe (Fase 49); escrita em LHS e execução real de memória pendentes.
5. **Inline assembly (`sussurro`)** — sem falar com CPU diretamente, nada de kernel.

---

## Observações de maturidade
- A nomenclatura sugerida (ex.: `seta`, `ninho`, `leque`, `passeio`, `sussurro`) segue o estilo do `docs/vocabulario.md` como referência auxiliar.
- Este backlog é deliberadamente separado das fases curtas atuais para evitar inflar escopo do Pinker v0.
- Itens marcados com 🔶 (parcial) têm implementação real no pipeline mas não estão operacionais em runtime ou têm escopo semântico incompleto.
