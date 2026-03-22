# Backlog futuro (visão, não compromisso imediato)

> **Nota de precedência**: `docs/future.md` é inventário amplo de possibilidades e backlog de longo prazo.
> A ordem oficial ativa de implementação está em `docs/roadmap.md`.
> Este documento **não é roadmap**, não dita ordem ativa e não substitui `docs/roadmap.md`.
>
> **Distinção importante**: este arquivo é inventário técnico estruturado — itens com status de implementação, organizados por camadas de dependência.
> `docs/parallel.md` é diferente: é o documento visionário da Pinker (fantasia orientadora, identidade, sonhos); não contém itens técnicos com status, não é backlog e não compete com este arquivo.

> **Trilha ativa atual**: **Bloco 7 — Backend nativo real** (definido em `docs/roadmap.md`).
> O Bloco 6 — Memória operacional foi concluído (Fases 64–72). O Bloco 7 é a trilha ativa corrente (Fases 73–83 já entregues).
> O **Bloco 8 — I/O e ecossistema útil** está definido como bloco futuro planejado, mas não ativo.
> Itens como terminal próprio, self-hosting, backend nativo completo, kernel real robusto, package manager e ecossistema soberano permanecem aqui como visão de longo prazo, sem competir com a trilha ativa.

Este documento organiza possibilidades futuras de evolução do Pinker v0 em camadas de dependência.
Cada item tem status: ✅ implementado, 🔶 parcial, ou sem marcação (não iniciado / ideia futura).

> **Status geral**: backlog estruturado para referência técnica; não é roadmap rígido de curto prazo.

---

## Camada 0 — Sistema de tipos (bloqueante para tudo)
- ~~Tipos inteiros com tamanho: `u8`–`u64`, `i8`–`i64`.~~ ✅ Implementado (Fases 43–44 + Fase 64). Tipos signed têm representação correta e execução real no runtime desde a Fase 64.
- ~~Tipo ponteiro (`seta`): `seta<T>`.~~ 🔶 Tipo implementado (Fase 48); representação mínima de ponteiro no runtime (Fase 65); operações parcialmente entregues (Fases 66–68, ver Camada 1). Acesso a outros tipos base além de `bombom`, semântica completa e backend nativo pendentes.
- ~~Structs (`ninho`): dados compostos com layout de memória definido.~~ 🔶 Declaração + semântica + layout estático (Fase 47 + Fase 51). Acesso a campo em leitura semântica (Fase 49); acesso operacional mínimo de campo via ponteiro em `--run` (Fase 69, subset `(*ptr).campo` escalar). Escrita em campo, acesso por valor (`p.campo`), construtor literal e campos não escalares pendentes.
- Enums (`leque`): variantes com ou sem dados associados.
- ~~Arrays de tamanho fixo: `[bombom; 256]`.~~ 🔶 Tipo na AST/semântica (Fase 46). Indexação em leitura semântica (Fase 49); indexação operacional mínima via ponteiro em `--run` (Fase 70, subset `(*ptr)[i]` com `[bombom; N]` e índice `bombom`). Escrita por índice, base por valor (`arr[i]`), bounds-check e elementos não `bombom` pendentes.
- Slices / arrays dinâmicos: referência a faixa de memória contígua.
- Tipo float (`bolha`): não crítico para kernel, mas útil para completude de médio nível.
- Tipo char/byte (`letra`/`migalha`): manipulação de texto e dados brutos.
- ~~Strings (`verso`): pelo menos como slice de bytes.~~ 🔶 Parcialmente implementado (Fase 61). Tipo `verso` e literal de string no frontend/semântica/IR; `falar("literal")` funciona em `--run`. Operações gerais de string (concatenação, comprimento, acesso por índice, passagem por chamada) pendentes.
- Tuples (`par`): retornos múltiplos e agrupamento leve.
- ~~Type alias (`apelido`): ex. `apelido Endereco = u64`.~~ ✅ Implementado (Fase 45).
- Void pointer / ponteiro genérico: equivalente a `void*` em C.

---

## Camada 1 — Operações de memória (bloqueante para kernel)
- ~~Dereferência de ponteiro: ler/escrever pelo endereço (`*seta`).~~ 🔶 Parcialmente implementado (Fases 66–67). Dereferência de leitura (`*p`) e escrita indireta (`*p = valor`) para `seta<bombom>` em `--run`. Acesso a outros tipos base além de `bombom`, backend nativo e semântica completa pendentes.
- ~~Aritmética de ponteiros: `seta + offset`, percorrer memória/arrays.~~ 🔶 Parcialmente implementado (Fase 68). `seta<bombom> + bombom` e `seta<bombom> - bombom` em `--run`. Fora de escopo: `n + ptr`, `ptr - ptr`, comparação rica de ponteiros e bases além de `bombom`.
- ~~Cast entre tipos (`virar`): converter inteiros entre tamanhos.~~ 🔶 Parcialmente implementado (Fase 50 + Fase 71). Cast inteiro→inteiro no frontend/semântica/IR (Fase 50); lowering operacional em CFG/Machine/runtime para inteiro→inteiro e `bombom <-> seta<bombom>` (Fase 71). Cast `seta<T> -> bombom` para `T != bombom`, cast geral entre ponteiros/compostos e backend nativo de cast pendentes.
- ~~Acesso a campos de struct por offset: `registro.campo`.~~ 🔶 Parcialmente implementado (Fase 49 + Fase 69). Leitura semântica e representação na IR (Fase 49); acesso operacional mínimo de campo via ponteiro em `--run` com offsets de layout estático (Fase 69, subset `(*ptr).campo` escalar). Escrita em campo, acesso por valor (`p.campo`), campos não escalares e lowering completo de memória pendentes.
- ~~Acesso a array por índice: `vetor[i]`.~~ 🔶 Parcialmente implementado (Fase 49 + Fase 70). Leitura semântica e representação na IR (Fase 49); indexação operacional mínima via ponteiro em `--run` (Fase 70, subset `(*ptr)[i]` com `[bombom; N]` e índice `bombom`). Escrita por índice, base por valor (`arr[i]`), bounds-check e elementos não `bombom` pendentes.
- ~~Volatile read/write (`fragil`): leitura/escrita que compilador não remove.~~ 🔶 Parcialmente implementado (Fase 52 + Fase 72). Qualificador semântico `fragil seta<T>` propagado no pipeline (Fase 52); efeito operacional mínimo em `deref_load`/`deref_store` com caminhos distintos `deref_*_fragil` vs `deref_*` em IR/CFG/selected/Machine/runtime para `fragil seta<bombom>` (Fase 72). MMIO real, fences, ordenação de memória e backend nativo pendentes.
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
- ~~Imports (`trazer`) de símbolos entre módulos.~~ 🔶 Parcialmente implementado (Fase 60). Import de módulo e símbolo no mesmo diretório do arquivo raiz, para `carinho` e `eterno`. Compilação separada plena, imports de outros diretórios, resolução de módulos aninhados e sistema de módulos completo pendentes.
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
- ~~Geração de código para x86_64.~~ 🔶 Parcialmente implementado (Fases 73–83, Bloco 7 ativo). Subset linear real: `principal() -> bombom` com locais, aritmética, chamadas diretas (até 2 parâmetros `bombom`), memória de frame via `%rbp` (Linux x86_64 hospedado). Controle de fluxo geral, memória indireta/ponteiros, globais, ABI completa e backend não hospedado pendentes.
- Emissão de binário ELF (ou flat binary).
- Register allocation.
- ~~Calling convention / ABI.~~ 🔶 Parcialmente implementado (Fases 74–76). ABI mínima real no subset: `%rdi` (arg0), `%rsi` (arg1), `%rax` (retorno), `%r10` (temporário volátil), frame `%rbp` com slots lineares. ABI completa de plataforma, 3+ parâmetros, tipos não `bombom` e register allocation amplo pendentes.
- Linker para combinar objetos compilados separadamente.
- Otimizações básicas: constant folding, dead code elimination, inlining.
- Segundo target opcional (RISC-V ou AArch64).

---

## Camada 6 — Capacidades de sistemas (bloqueante para kernel)
- ~~Inline assembly (`sussurro`).~~ 🔶 Parcialmente implementado (Fase 56). Statement textual `sussurro("...")` preservado no frontend/semântica/IR. Lowering operacional em CFG/Machine/runtime e emissão real de instruções de CPU pendentes.
- Atributos de função (`#[naked]`, `#[no_mangle]`, `#[interrupt]`).
- ~~No-std / freestanding.~~ 🔶 Parcialmente implementado (Fase 57). Marca de unidade `livre;` reconhecida no pipeline como intenção freestanding. Runtime bare-metal executável real e suporte operacional pleno pendentes.
- ~~Linker script support.~~ 🔶 Parcialmente implementado (Fase 58). Linker script textual mínimo emitido em `--asm-s` para unidades `livre;` (`ENTRY(_start)` + seções básicas). Linker script real para kernel bootável e suporte de runtime bare-metal pendentes.
- Constantes em tempo de compilação (const eval).
- Static mutável (`raiz mut`).
- Union / reinterpret cast.
- Packed structs.

---

## Camada 7 — Biblioteca padrão mínima (necessário para self-hosting)
- ~~I/O básico (`falar`/`ouvir`).~~ 🔶 Parcialmente implementado (Fase 62). `falar(expr)` em `--run` para `bombom`, `u8`–`u64`, `logica` e `verso`. `ouvir` (leitura), arquivo e formatação avançada pendentes.
- Manipulação de arquivos (`abrir`/`fechar`/`escrever`).
- ~~Strings com operações.~~ 🔶 Parcialmente implementado (Fase 61). Tipo `verso` e literal de string integrados ao frontend/semântica/IR; `falar("literal")` funciona em `--run`. Passagem por chamada, retorno, variável geral, concatenação, comprimento e acesso por índice pendentes.
- Coleções básicas (`Vec`, `HashMap`).
- Formatação de texto.
- Alocador de heap.
- Tratamento de erros (`amparo`/`tropeco`), equivalente a `Result`/`Option`.

---

## Camada 8 — Infraestrutura de build (necessário para self-hosting)
- ~~Sistema de build próprio ou integrado para múltiplos `.pink`.~~ 🔶 Parcialmente implementado (Fase 63). `pink build <arquivo.pink>` gera artefato textual `.s` em disco. Build para múltiplos arquivos, resolução de dependências e sistema de build completo pendentes.
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

1. **Backend x86_64** — 🔶 em progresso (Bloco 7, Fases 73–83). Subset linear real já montável (locais, aritmética, chamadas, memória de frame). Controle de fluxo geral, memória indireta/ponteiros, globais e ABI completa ainda pendentes.
2. **Dereferência de ponteiro + aritmética** — 🔶 parcial (Fases 66–68, subset `seta<bombom>`). Leitura (`*p`), escrita indireta (`*p = valor`) e aritmética (`ptr ± n`) operacionais em `--run`. Acesso a outros tipos base, backend nativo e operações avançadas pendentes.
3. **Cast operacional (`virar`)** — 🔶 parcial (Fase 50 + Fase 71). Inteiro→inteiro e `bombom <-> seta<bombom>` operacionais em `--run`. Cast `seta<T> -> bombom` genérico, casts entre compostos e backend nativo pendentes.
4. **Acesso a campo e indexação com escrita + runtime** — 🔶 parcial (Fases 69–70). Leitura operacional de campo via ponteiro (`(*ptr).campo`) e indexação via ponteiro (`(*ptr)[i]`) em `--run` para subset escalar/`bombom`. Escrita em campo/índice, base por valor, elementos não escalares pendentes.
5. **Inline assembly (`sussurro`)** — 🔶 parcial (Fase 56). Statement textual preservado até IR. Lowering operacional em CFG/Machine/runtime e emissão de instruções reais de CPU pendentes.

---

## Observações de maturidade
- A nomenclatura sugerida (ex.: `seta`, `ninho`, `leque`, `passeio`, `sussurro`) segue o estilo do `docs/vocabulario.md` como referência auxiliar.
- Este backlog é deliberadamente separado das fases curtas atuais para evitar inflar escopo do Pinker v0.
- Itens marcados com 🔶 (parcial) têm implementação real no pipeline mas não estão operacionais em runtime ou têm escopo semântico incompleto.
