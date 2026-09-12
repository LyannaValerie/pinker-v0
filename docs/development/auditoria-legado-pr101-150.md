# Auditoria do legado da Pinker — era dos PRs #101–150

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Terceiro documento da série. Cobre os **PRs #101 a #150**, mergeados entre **20 e
23 de março de 2026**.

Pré-requisitos: `auditoria-legado-inicial.md` (#1–50) e
`auditoria-legado-pr51-100.md` (#51–100). O método e o registro de achados vivem
lá; aqui entram apenas os achados desta era.

---

## 1. A era: o interpretador ganha o mundo

Três dias e meio. O que entrou:

| PRs | Entrega |
|---|---|
| #101, #102 | signed real no runtime; representação de ponteiro no runtime |
| #106–#110 | dereferência de leitura; escrita indireta; aritmética de ponteiro; campo em `ninho`; indexação de array |
| #113, #114 | `virar` operacional ligado à memória; `fragil` com efeito real |
| #116–#131 | backend externo: frame, registradores, múltiplos params, memória real, e as recusas explícitas de `talvez`/`sempre que`/3+ params |
| #126 | `mut` → **`muda`** (identidade lexical) |
| #133–#137 | `ouvir` (stdin); `abrir`/`fechar` (arquivo); `escrever`; `verso` operacional |
| #138, #140, **#141** | `juntar_verso`, `tamanho_verso`, `indice_verso`, **`falar` com múltiplos argumentos** |
| #142–#147 | argv, `quantos_argumentos`, `argumento_ou`, ambiente de processo |
| #148 | Paralela-1: negação bitwise dual |

Sobrevivência: **4.472 linhas (2,21%)**, das quais **2.568 em `src/` (2,82%)**.

Diferente das duas eras anteriores, esta não deixou um arquivo-monumento. Deixou
uma camada fina espalhada por toda a pipeline — `interpreter.rs` (500),
`backend_s.rs` (469), `cfg_ir.rs` (296), `ir.rs` (243), `semantic.rs` (196),
`backend_text.rs` (128), `instr_select.rs` (123), `abstract_machine.rs` (113). É
o formato de quem estava atravessando a pipeline inteira, feature após feature.

Justamente por isso os dois achados desta era são sobre **forma**, não sobre
arquivo.

---

## 2. L-12 — `falar` é o último enclave da patologia que o projeto já curou

Este é o achado mais acionável da série até aqui, porque **o remédio já existe
dentro do repositório, adulto e testado**.

### 2.1 O que o projeto já resolveu

`src/intrinsics/registry.rs` abre com um diagnóstico que poderia estar nesta
auditoria:

> Antes da consolidação C1 o mesmo fato existia **sete vezes**: `semantic`, `ir`,
> `ir_validate`, `cfg_ir_validate`, `instr_select_validate`,
> `abstract_machine_validate` e `backend_s` enumeravam cada um a sua cópia, e
> nada impedia que discordassem.
>
> ```text
> REGISTRY                       = SOURCE OF DECLARATIVE TRUTH
> SEMANTIC/IR/BACKEND/VALIDATORS = CONSUMERS OR DERIVED VIEWS
> ```

A consolidação C1 (PR #589, `[MODULARIZAÇÃO][C1-C]`) transformou as intrínsecas
históricas numa autoridade declarativa única. O padrão é explicitamente
reconhecido como maduro e tem precedentes citados no próprio arquivo:
`falha_operacional`, `valor_json`, `sha256`, `saida_processo`.

### 2.2 O que ficou de fora

`falar` **não está no registry**. Nem em `registry.rs`, nem em `identity.rs`, nem
em `public_surface.rs`.

Ele nasceu na Fase 62 (era #51–100) como comando de linguagem e, na **Fase 91
(PR #141, 23/03, esta era)**, ao ganhar múltiplos argumentos, virou seis opcodes
da máquina abstrata:

```rust
MachineInstr::PrintIntInline,
MachineInstr::PrintBoolInline,
MachineInstr::PrintStrValueInline,
MachineInstr::PrintStrInline(String),
MachineInstr::PrintSpace,
MachineInstr::PrintNewline,
```

O inventário de navegação registra o estado, §7.6:

> **`falar`:** permanece instrução própria, com mapeadores de CFG e de seleção;
> não é intrínseca do backend.

### 2.3 O custo, medido

Comparação direta entre duas operações da mesma natureza — ambas são chamadas
que o usuário escreve, ambas produzem efeito, ambas atravessam a pipeline:

| Operação | Governança | Arquivos de `src/` que a enumeram |
|---|---|---:|
| `juntar_verso` | registry declarativo | **4** |
| `falar` | instrução própria em todas as camadas | **20** |

Os vinte: `ast`, `token`, `lexer`, `parser/comandos`, `parser/genericos`,
`printer`, `semantic`, `module_resolve`, `ir`, `ir_validate`, `cfg_ir`,
`cfg_ir_validate`, `instr_select`, `instr_select_validate`, `abstract_machine`,
`abstract_machine_validate`, `interpreter`, `backend_text`,
`backend_text_validate`, `backend_s`.

Cinco vezes a superfície, pela mesma capacidade, por uma decisão de março.

E a conta continua correndo: quando `falar` ganhar formatação, largura, saída
para `stderr` ou destino configurável — coisas que a Faixa 11 (I/O) do Bloco 20
vai pedir —, serão vinte arquivos, não quatro.

### 2.4 Recomendação

**Migrar `falar` para o registry declarativo, reusando o padrão C1.** Não é
reforma nova: é aplicar ao último enclave o mesmo tratamento que sete
subsistemas já receberam e que tem precedente adulto documentado no próprio
arquivo.

Prioridade: **depois** da poda de L-01 e do teste de concordância de L-08, e
**antes** de qualquer fase da Faixa 11. Fazer isso durante uma fase de I/O
significaria pagar o pedágio de vinte arquivos mais uma vez.

Limite honesto: os seis opcodes `Print*Inline` da máquina abstrata não somem com
a migração — a máquina precisa continuar sabendo imprimir. O que a migração
elimina é a **enumeração declarativa** de `falar` em semântica, IR e validadores.
Uma estimativa honesta reduz de 20 para algo em torno de 8–10 arquivos, não para
4. Ainda assim, é metade.

---

## 3. L-13 — nove espaços de handle, uma só defesa

A Fase 86 (PR #135) introduziu handles de arquivo no interpretador. O padrão
pegou. Hoje o interpretador mantém, em estruturas separadas, **pelo menos nove
contadores de handle independentes**:

```
next_file_handle: 1        next_map_handle: 1         next_handle: 1   (callables)
next_list_handle: 1        next_map_iter_handle: 1    next_handle: 1   (vtables/objetos)
next_generator_handle: 1   next_enum_handle: 1        next_vtable_handle: 1
```

mais três espaços de endereço sintético: `0x1000_0000` (alocações),
`0x2000_0000` (dados), `0x7000_0000` (slots).

Consequência: **o valor `1` é simultaneamente** arquivo 1, lista 1, mapa 1,
iterador 1, leque 1, gerador 1, callable 1 e vtable 1. Nada no número distingue
as famílias. A única defesa é a variante nominal de `RuntimeValue` — o achado
L-04.

Não é especulação minha que isso preocupa: os comentários do próprio código
dizem, sobre `SaidaProcesso` e `ValorJson`:

> Categoria distinta de `SaidaProcesso` de propósito: as duas são handles de uma
> palavra, mas confundi-las faria um acessor de uma família aceitar valor da
> outra.

e, sobre `SaidaProcesso`:

> Handle nominal de snapshot de processo. **Nunca é confundido com lista.**

São afirmações de intenção, garantidas por revisão humana e pela disciplina de
casar a variante certa em cada `match`. Não há discriminante no valor, nem
verificação em tempo de execução, nem teste que exercite a confusão.

**Classificação honesta:** risco latente, não defeito. O modelo funciona hoje. O
que ele não tem é uma rede — e o Bloco 20 vai esticá-lo: a Faixa 6 pede
iteradores lazy (item 23), a Faixa 10 pede threads e canais (itens 42–45), a
Faixa 11 pede descritores de dispositivo (itens 51–52). Cada um é uma família
nova de handle de uma palavra começando em 1.

**Melhoria conservadora possível:** um único espaço de handle com tag de família
embutida nos bits altos, ou — mais barato ainda e sem mudar representação —
contadores que **começam em bases distintas por família**, de modo que um handle
trocado falhe alto em vez de acertar silenciosamente. A segunda opção é dezenas
de linhas e nenhuma mudança de contrato observável.

Não recomendo executar nenhuma das duas agora: recomendo que a decisão seja
tomada **antes** da primeira fase da Faixa 10, não durante.

---

## 4. O que esta era **não** deixou

Vale registrar pelo valor negativo. As "recusas explícitas" do backend externo
(Fases 81–84: recusa de 3+ parâmetros, de `talvez/senao`, de `sempre que`) foram
escritas nesta era como limites conscientes — e **não sobreviveram como
recusas**: o Eixo B as substituiu por implementação real entre as Fases 212 e
222.

Esse é o contraste que dá sentido ao resto da série. Quando o projeto decidiu
encarar um limite de frente, o limite sumiu. Os achados L-01 a L-13 são
exatamente os limites que **nunca** foram encarados de frente — não porque
alguém decidiu mantê-los, mas porque nunca chegou a hora de decidir.

---

## 5. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-12** | `falar` fora do registry: 20 arquivos contra 4 de uma intrínseca governada | #101–150 | fricção com remédio pronto |
| **L-13** | Nove espaços de handle independentes, defendidos só pela variante nominal | #101–150 | risco latente |
