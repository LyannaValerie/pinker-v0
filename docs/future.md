# Backlog futuro (inventário amplo — não é roadmap ativo)

> **Precedência documental**: este arquivo é inventário amplo de possibilidades.
> A ordem oficial ativa de implementação está em `docs/roadmap.md`.
> Itens aqui **não ditam prioridade nem sequência** — servem como referência e memória de projeto.

> Revisado em rodada documental paralela à Fase 51 (sem número de fase).
> Status de cada item atualizado para refletir o estado real após a Fase 50.

---

## Convenção de status

- ~~`item`~~ ✅ = implementado por completo no pipeline ativo
- `item` ⚠️ parcial = núcleo entrou, mas partes relevantes ainda pendentes (indicadas inline)
- `item` = não iniciado / ideia futura

---

## Camada 0 — Sistema de tipos (bloqueante para tudo)

- ~~Tipos inteiros com tamanho: `u8`–`u64`, `i8`–`i64`.~~ ✅ Implementado. Runtime signed bloqueado (representação u64-only — decisão explícita de HF-3).
- ~~Tipo ponteiro (`seta`): `seta<T>`.~~ ✅ Tipo no pipeline (frontend/semântica/IR). Dereferência e runtime operacional pendentes.
- ~~Structs (`ninho`): tipo nomeado composto.~~ ✅ Declaração + semântica implementadas. Layout de memória, construtor literal e runtime operacional pendentes.
- ~~Arrays de tamanho fixo: `[tipo; N]`.~~ ✅ Tipo no pipeline (frontend/semântica/IR). Runtime/acesso por índice operacional pendente.
- ~~Type alias (`apelido`): ex. `apelido Endereco = u64`.~~ ✅ Implementado por completo (frontend/semântica/IR/lowering).
- Enums (`leque`): variantes com ou sem dados associados. Não iniciado.
- Slices / arrays dinâmicos: referência a faixa de memória contígua. Não iniciado.
- Tipo float (`bolha`): não crítico para kernel, útil para completude de médio nível. Não iniciado.
- Tipo char/byte (`letra`/`migalha`): manipulação de texto e dados brutos. Não iniciado.
- Strings (`verso`): pelo menos como slice de bytes. Não iniciado.
- Tuples (`par`): retornos múltiplos e agrupamento leve. Não iniciado.
- Void pointer / ponteiro genérico: equivalente a `void*` em C. Não iniciado.

---

## Camada 1 — Operações de memória (bloqueante para kernel)

- ~~Cast entre tipos (`virar`): converter inteiros entre tamanhos.~~ ⚠️ parcial: frontend/semântica/IR implementados (Fase 50). CFG/Machine/runtime ainda não loweram/executam cast.
- Acesso a campos de struct por offset (`registro.campo`). ⚠️ parcial: leitura semântica/IR implementada (Fase 49). CFG/Machine/runtime não loweram acesso a campo.
- Acesso a array por índice (`vetor[i]`). ⚠️ parcial: leitura semântica/IR implementada (Fase 49). CFG/Machine/runtime não loweram indexação.
- Dereferência de ponteiro: ler/escrever pelo endereço (`*seta`). Não iniciado.
- Aritmética de ponteiros: `seta + offset`, percorrer memória/arrays. Não iniciado.
- Volatile read/write (`frágil`): leitura/escrita que compilador não remove. Não iniciado.
- Alocação manual (`reserva`/`soltar`) ou chamada explícita a alocador. Não iniciado.
- `sizeof` (`peso`): tamanho de tipo em bytes em tempo de compilação. Não iniciado.
- Align/alinhamento: controle de alinhamento de structs e campos. Não iniciado.

---

## Camada 2 — Controle de fluxo e expressividade

- ~~`sempre que` (while loop) com `quebrar` e `continuar`.~~ ✅ Implementado por completo (Fases 27–30).
- ~~Operadores lógicos `&&` e `||` com short-circuit.~~ ✅ Implementado (Fase 36).
- ~~Operadores bitwise: `&`, `|`, `^`, `<<`, `>>`.~~ ✅ Implementado (Fase 34).
- `for` / iteração (`passeio`). Não iniciado.
- `match` / pattern matching (`encaixe`). Não iniciado.
- Operadores compostos: `+=`, `-=`, `<<=`, `>>=`, `&=`, `|=`. Não iniciado.
- Loop infinito (`roda`) com bloco explícito. Não iniciado.
- Expressões como valor (estilo Rust), se fizer sentido. Ideia futura.

---

## Camada 3 — Sistema de módulos e organização (bloqueante para self-hosting)

- Módulos (`canto`) para separar código em arquivos. Não iniciado.
- Imports (`trazer`) de símbolos entre módulos. Não iniciado.
- Visibilidade (`aberto`/`segredo`). Não iniciado.
- Compilação separada + linkagem. Não iniciado.
- Namespaces / resolução qualificada (`modulo.funcao()` ou `modulo::funcao()`). Não iniciado.

---

## Camada 4 — Polimorfismo e abstração (necessário para self-hosting elegante)

- Traits/interfaces (`molde`). Não iniciado.
- Impl blocks (`vestir`). Não iniciado.
- Generics (`qualquer`) em funções/tipos. Não iniciado.
- Ponteiros de função / closures. Não iniciado.

---

## Camada 5 — Backend de código nativo (bloqueante para tudo executável)

- Geração de código para x86_64. Não iniciado.
- Emissão de binário ELF (ou flat binary). Não iniciado.
- Register allocation. Não iniciado.
- Calling convention / ABI. Não iniciado.
- Linker para combinar objetos compilados separadamente. Não iniciado.
- Otimizações básicas: constant folding, dead code elimination, inlining. Não iniciado.
- Segundo target opcional (RISC-V ou AArch64). Ideia futura.

---

## Camada 6 — Capacidades de sistemas (bloqueante para kernel)

- Inline assembly (`sussurro`). Não iniciado.
- Atributos de função (`#[naked]`, `#[no_mangle]`, `#[interrupt]`). Não iniciado.
- No-std / freestanding. Não iniciado.
- Linker script support. Não iniciado.
- Constantes em tempo de compilação (const eval). Não iniciado.
- Static mutável (`raiz mut`). Não iniciado.
- Union / reinterpret cast. Não iniciado.
- Packed structs. Não iniciado.

---

## Camada 7 — Biblioteca padrão mínima (necessário para self-hosting)

- I/O básico (`falar`/`ouvir`). Não iniciado.
- Manipulação de arquivos (`abrir`/`fechar`/`escrever`). Não iniciado.
- Strings com operações. Não iniciado.
- Coleções básicas (`Vec`, `HashMap`). Não iniciado.
- Formatação de texto. Não iniciado.
- Alocador de heap. Não iniciado.
- Tratamento de erros (`amparo`/`tropeco`), equivalente a `Result`/`Option`. Não iniciado.

---

## Camada 8 — Infraestrutura de build (necessário para self-hosting)

- Sistema de build próprio ou integrado para múltiplos `.pink`. Não iniciado.
- Preprocessador ou macro system (opcional). Ideia futura.
- Testes integrados do compilador sobre o próprio compilador. Não iniciado.

---

## Distância estimada (referência orientativa, não compromisso)

- Linguagem de uso geral → Camadas 0 + 2 + 3 → ~33 itens restantes.
- Programação de sistemas → + 1 + 5 + 6 → ~52 itens restantes.
- Criar um kernel → + 6 completa → ~52 itens restantes.
- Self-hosting (compilar a si mesma) → + 3 + 4 + 7 + 8 → ~62 itens (todos).

> Estes números são aproximações para referência de escala, não contagem precisa.
> A ordem de execução é determinada por `docs/roadmap.md`, não por este inventário.

---

## Observações de maturidade

- A nomenclatura sugerida (ex.: `seta`, `ninho`, `leque`, `passeio`, `sussurro`) segue o estilo de `docs/vocabulario.md` como referência auxiliar.
- Este backlog é deliberadamente separado das fases curtas ativas para evitar inflar escopo do Pinker v0.
