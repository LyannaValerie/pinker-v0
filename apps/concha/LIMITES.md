# O que a concha não consegue fazer — e a prova disso no fonte da Pinker

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Este documento é o resultado real do experimento "recriar o bash usando só a
Pinker". A shell existe e funciona ([`README.md`](README.md)); o que interessa
aqui é a fronteira: cada coisa que um shell precisa fazer e a Pinker ainda não
deixa, com **repro executável**, **a mensagem exata que a Pinker emite** e **o
ponto do fonte Rust que produz aquela mensagem**.

Duas categorias, deliberadamente separadas:

- **L — limite de superfície.** A capacidade não existe ainda. A Pinker recusa,
  e recusa dizendo por quê. É trilha, não bug.
- **D — defeito.** A Pinker aceita e faz a coisa errada, ou aceita num backend e
  quebra no outro. Nenhum deles estava coberto por teste ou exemplo do
  repositório antes deste experimento.

Todos os repros estão em [`limites/`](limites/) e rodam assim:

```bash
cargo run --bin pink -- --run   apps/concha/limites/<arquivo>.pink
cargo run --bin pink -- --check apps/concha/limites/<arquivo>.pink
PINKER_RT_LIB=target/release/libpinker_rt.a \
  cargo run --bin pink -- build --nativo --out-dir /tmp/nat apps/concha/limites/<arquivo>.pink
```

## Resumo

| # | Falta | Efeito na shell | A Pinker avisa? |
|---|---|---|---|
| L-01 | `mudar_diretorio` | `cd` é virtual; a shell nunca sai do diretório onde nasceu | sim, na semântica |
| L-02 | argv arbitrário nas intrínsecas históricas | mitigado por `executar_processo_estruturado` | sim, na semântica |
| L-03 | fluxo contínuo entre processos | `yes \| head -1` trava para sempre | não — trava calada |
| L-04 | stderr como fluxo | tudo sai pela saída padrão | não |
| L-05 | `fork` / job control | `&`, `jobs`, `fg`, `bg`, `wait` impossíveis | sim, na semântica |
| L-06 | status de filho morto por sinal | `128+N` do bash não existe | sim, como valor de falha |
| L-07 | terminal para o filho | `vim`, `less`, `top`, `ssh` não funcionam | não — o filho é que descobre |
| L-08 | saída binária | pipeline binário é recusado | sim, como valor de falha |
| L-09 | saída sem quebra de linha | sem `echo -n` real, sem prompt na mesma linha | não |
| L-10 | EOF distinguível | fim de entrada é adivinhado por sentinela | não |
| L-11 | ordem entre `verso` | glob precisa de tabela de caracteres escrita à mão | sim, na semântica |
| L-12 | convenção única de índice | `indice_verso_em` devolve byte, `indice_verso` quer codepoint | às vezes: estoura ou corrompe |
| L-13 | criação de arquivo falível | `> /caminho/ruim` derruba a shell inteira | sim, mas fatal |
| L-14 | identidade do processo | sem `$$`, sem `$!` | sim, na semântica |
| L-15 | escapes de string além de `\n \t \r \0 \\ \"` | sem `\e`/`\u`: cor ANSI só via `printf` externo | sim, no léxico |
| L-16 | PATH herdado | filho só vê PATH fixa do runtime, salvo overlay | não |
| L-17 | `ouvir*` no backend nativo | shell nativa não lê a entrada padrão | sim, mas sem nomear a função |
| L-18 | `!` no backend nativo | negação lógica não monta | sim, mas sem nomear o operador |
| D-01 | `sempre que` com `&&`/`\|\|` | laço infinito | **não** |
| D-02 | literal `verso` com `\0` no nativo | assembly inválido | **não** (quem reclama é o `as`) |
| D-03 | `logica == logica` no interpretador | derruba em tempo de execução | só no runtime |
| D-04 | `criar_arquivo` sobre arquivo existente | interpretador trunca, nativo aborta | só no runtime nativo |

---

# Parte 1 — limites de superfície

## L-01 — a shell não muda de diretório

**bash:** `cd` chama `chdir(2)` e o processo inteiro muda de lugar.
**Pinker:** existe `diretorio_atual()` para ler; não existe nada para escrever.

Repro: `limites/l01_sem_mudar_diretorio.pink`

```
Erro Semântico: função 'mudar_diretorio' não declarada em 7:5..7:20
  |     mudar_diretorio("/tmp");
  |     ^
```

**Como a concha contorna:** mantém um `PWD` próprio no estado, resolve todo
caminho relativo à mão (`resolver`/`normalizar_caminho`) e entrega o diretório
para cada filho no parâmetro `diretorio` de `executar_processo_estruturado`.
O efeito visível é quase idêntico — mas qualquer intrínseca de caminho relativo
(`listar_diretorio("."`)) continua olhando para o diretório real do processo,
não para o `PWD` da concha. É por isso que a concha nunca passa caminho relativo
para uma intrínseca sem antes resolvê-lo.

## L-02 — as intrínsecas históricas de processo têm argv de tamanho fixo

**Repro:** `limites/l02_argv_fixo.pink`

```
Erro Semântico: chamada de 'executar_processo' com aridade inválida: esperado 1 ou 2, recebido 3
```

Fonte: `src/semantic.rs`, ramo `name == "executar_processo"` (aridade 1..=2);
o mesmo padrão vale para `capturar_stdout`, `capturar_stderr` e
`executar_com_entrada` (um argv explícito, no máximo).

**Estado real:** este limite está *resolvido* pela superfície estruturada da
Parte D — `executar_processo_estruturado(programa, argv: lista<verso>, entrada,
diretorio, ambiente: mapa<verso,verso>, LimiteTempo)` — que é o que a concha
usa. Fica registrado porque é a primeira parede que aparece para quem tenta
escrever uma shell com o vocabulário histórico.

## L-03 — não existe fluxo contínuo entre processos

Repro: `limites/l03_sem_fluxo_continuo.pink` — **não termina**, rode com `timeout 5`.

**bash:** `yes | head -1` imprime uma linha e termina: `head` sai, `yes` toma
SIGPIPE e morre.
**Pinker:** um estágio só começa quando o anterior terminou. `SaidaProcesso`
só passa a existir depois do `wait`:

> "Quando o handle passa a existir, o filho já foi esperado e reapado e os pipes
> já foram fechados." — `src/saida_processo.rs:37`

Consequência medida na concha:

```
$ concha --comando 'yes | head -1'      # nunca termina
$ concha --comando 'seq 1 5 | tail -2'  # funciona: 4 e 5
```

**Este é o limite mais caro do experimento**, e o único da lista que não emite
mensagem nenhuma: o programa simplesmente não volta. Um pipeline com produtor
infinito é indistinguível de um travamento.

## L-04 — não existe stderr como fluxo

Não há intrínseca que escreva no descritor 2. `falar` é o único caminho de saída
de texto da linguagem, e vai para a saída padrão
(`MachineInstr::PrintStrValueInline` → `print!` em `src/interpreter.rs`).

A concha até separa os dois canais internamente (captura `processo_erro` de cada
filho e implementa `2>`), mas quando precisa mostrar um diagnóstico, ele sai
misturado com a saída normal. `concha --comando 'ls /nada' 2> erro.txt` funciona;
`concha ... 2>/dev/null` no shell de fora não filtra nada.

## L-05 — não existe `fork` nem controle de jobs

Repro: `limites/l05_sem_fork.pink`

```
Erro Semântico: função 'executar_em_segundo_plano' não declarada
```

Não há superfície para criar processo sem esperar por ele. `&`, `jobs`, `fg`,
`bg`, `wait`, `disown` e `Ctrl-Z` ficam todos fora. A concha reconhece `&` no
tokenizador só para poder recusar com uma frase honesta em vez de fingir:

```
$ concha --comando 'sleep 5 & echo depois'
concha: '&' pede processo em segundo plano; a Pinker nao expoe fork nem controle de jobs (limite L-05)
```

## L-06 — filho morto por sinal não vira status

**bash:** `sh -c 'kill -9 $$'` devolve 137 (`128+9`).
**Pinker:** vira falha operacional, não status.

Repro: `limites/l06_filho_morto_por_sinal.pink`

```
ERRO: processo estruturado terminou sem código normal; nenhum código mágico foi fabricado
```

Fonte: `src/processo_estruturado_hospedado.rs:263`. A recusa é deliberada e está
escrita no próprio texto do erro — a Pinker prefere não inventar número a
inventar. Para a shell, porém, o efeito é que morte por sinal e "não deu para
executar" chegam pelo mesmo canal (`Resultado.Erro`), e a distinção se perde.

## L-07 — o filho nunca recebe um terminal

Repro: `limites/l07_sem_terminal.pink`

```
stdin_e_terminal=1
stdout_e_terminal=1
(0 = verdadeiro no test do shell; 1 = falso)
```

Fonte: `src/processo_estruturado_hospedado.rs:69-71` — `stdin`, `stdout` e
`stderr` são **sempre** `Stdio::piped()`. Não há como herdar o terminal.

Consequência: `vim`, `less`, `top`, `ssh`, `htop`, qualquer coisa que teste
`isatty` ou queira modo cru não funciona sob a concha. Programas que só mudam
de cor com tty apenas perdem a cor.

## L-08 — a saída do filho precisa ser UTF-8

Repro: `limites/l08_saida_binaria.pink`

```
ERRO: stdout do processo estruturado não é UTF-8 válido
```

Fonte: `src/processo_estruturado_hospedado.rs:267-269`.

`verso` é texto, não bytes, e a fronteira é estrita. Isso mata qualquer pipeline
binário (`head -c 8 /dev/urandom | xxd`, `tar | gzip`, `cat foto.png | ...`) —
e mata *no meio*, com o filho já executado.

## L-09 — `falar` sempre quebra linha

Repro: `limites/l09_falar_sempre_quebra_linha.pink`. `falar` junta argumentos com
espaço e termina com `PrintNewline`; não existe forma de escrever texto sem
quebra na saída padrão.

Duas consequências para a shell:

- o prompt sai numa linha só dele — a concha imprime `[/dir]$` e o que você
  digita aparece embaixo;
- `echo -n` da concha existe (ela guarda a saída do estágio e só imprime no fim
  do pipeline), mas quando a saída chega ao terminal, a quebra final é da
  linguagem, não da shell.

## L-10 — não dá para distinguir fim de entrada

`ouvir_verso_ou(padrao)` devolve o padrão **tanto no EOF quanto em erro de
leitura** — o mesmo braço trata os dois casos (`src/interpreter.rs:4561`:
`Ok(None) | Err(_) => ...`).

A concha contorna com um sentinela improvável (`@@concha-fim-de-entrada@@`):
se a linha lida for exatamente igual ao sentinela, ela assume Ctrl-D. É um
palpite bem informado, não uma detecção.

## L-11 — `verso` não tem ordem nem código de caractere

Repro: `limites/l11_verso_sem_ordem.pink`

```
Erro Semântico: função 'menor_verso' não declarada
```

Existem `igual_verso`, `contem_verso`, `comeca_com`, `termina_com` — nenhuma
comparação de ordem, e nenhuma conversão caractere → número. O glob do shell
precisa devolver nomes ordenados (`listar_diretorio` devolve na ordem do
filesystem), então a concha carrega uma **tabela de caracteres escrita à mão**
(`tabela_ordem`) e ordena por posição nessa tabela. Fora dela, a ordem é
arbitrária: acentos e qualquer coisa não-ASCII caem todos no mesmo balde.

## L-12 — duas convenções de índice que não conversam

Repro: `limites/l12_indice_em_bytes.pink`

```
tamanho_verso (codepoints): 6
indice_verso_em '=' (bytes): 6
Erro Runtime: índice fora da faixa em 'indice_verso' para o verso informado
```

- `tamanho_verso`, `indice_verso`, `fatiar_verso` → **codepoints**
  (`src/interpreter.rs:4922`, `chars().count()` / `chars().nth()`);
- `indice_verso_em`, `buscar_verso` → **bytes**
  (`src/interpreter.rs:5180`, `str::find`).

Em texto ASCII ninguém percebe. Em texto com acento, o resultado ou estoura ou
corrompe **em silêncio**. Na concha, o teste de atribuição `NOME=valor` usa
`indice_verso_em` para achar o `=` e `fatiar_verso` para cortar:

```
$ concha --comando 'ação=doce; set'
ação=d=ce
```

O nome virou `ação=d` e o valor virou `ce`. Nenhum erro, nenhum aviso.

## L-13 — criar arquivo não é falha recuperável

Repro: `limites/l13_criar_arquivo_e_fatal.pink`

```
Erro Runtime: falha ao criar arquivo em 'criar_arquivo': No such file or directory (os error 2)
```

A camada de falha-como-valor existe e é ótima
(`src/falha_operacional.rs`, `SUPERFICIES_FALIVEIS`), mas cobre só seis
superfícies — entre elas `ler_arquivo_resultado`, e **nenhuma** de escrita. Um
`bash` responde `No such file or directory` e continua vivo; a concha morre
inteira, sessão e histórico junto:

```
$ concha --comando 'echo x > /naoexiste/y.txt'
Erro Runtime: ... falha ao criar arquivo ...
```

## L-14 — o processo não sabe quem é

Repro: `limites/l14_sem_pid.pink` → `função 'processo_id' não declarada`.

Sem `$$`, sem `$!`, sem PID do filho. Arquivo temporário nomeado por PID, lock
por PID e `kill` do próprio grupo ficam fora.

## L-15 — o léxico aceita poucos escapes

Repro: `limites/l15_escape_invalido.pink`.

`src/lexer.rs` reconhece exatamente `\n`, `\t`, `\r`, `\0`, `\\` e `\"`;
qualquer outro vira erro léxico com nome próprio:

```
Erro Léxico: sequência de escape inválida '\u' em 1079:10..1079:12
```

(foi o que aconteceu na primeira versão do sentinela de fim de entrada da
concha). Sem `\e` nem `\u`, a shell não emite sequência ANSI direto — prompt
colorido e limpeza de tela só delegando para `printf` externo.

## L-16 — a PATH dos filhos é fixada pelo runtime

Repro: `limites/l16_path_forcado.pink`

```
PATH_DO_FILHO=/usr/local/bin:/usr/bin:/bin
```

Fonte: `src/processo_estruturado_hospedado.rs:20` e `:68` — a PATH é sobrescrita
por uma constante **antes** do overlay do usuário. O filho nunca vê a PATH do
pai por herança; só vê o que a shell mandar explicitamente. A concha exporta
`PATH` a cada spawn justamente para reconstituir o comportamento esperado.

## L-17 — a família `ouvir*` não existe no backend nativo

Repro: `limites/l17_ouvir_sem_nativo.pink`

```
$ pink --run          -> lê e imprime
$ pink build --nativo -> Erro Validação Backend Textual: subset externo montável
                         (Fase 84) encontrou call para função inexistente em 1:1..1:1
```

Fonte: `runtime_intrinsic_symbol` em `src/backend_s.rs` (a tabela fechada de
intrínseca → símbolo do runtime nativo) não tem nenhuma entrada `ouvir*`;
a chamada cai no erro de `src/backend_s.rs:988`.

**Este é o achado mais bonito do experimento.** A concha inteira — tokenizador,
glob com ordenação, pipelines, redirecionamento, `se`/`enquanto`/`para`,
substituição de comando, spawn estruturado — compila para binário x86-64 real e
roda o `demo.concha` byte a byte igual ao interpretador. A única coisa que a
impede de ser uma shell nativa de verdade é **ler uma linha do teclado**:

```bash
sed 's/ouvir_verso_ou(sentinela_fim())/sentinela_fim()/; s/ouvir_verso_ou("")/""/' \
  apps/concha/principal.pink > /tmp/concha_sem_repl.pink
PINKER_RT_LIB=target/release/libpinker_rt.a \
  pink build --nativo --out-dir /tmp/concha_nat /tmp/concha_sem_repl.pink
/tmp/concha_nat/concha_sem_repl --roteiro apps/concha/demo.concha   # roda tudo
```

Uma única substituição separa a Pinker de ter uma shell compilada.

## L-18 — o operador `!` não monta no nativo

Repro: `limites/l18_nao_logico_sem_nativo.pink`

```
$ pink --run          -> imprime
$ pink build --nativo -> Erro Validação Backend Textual: subset externo montável (Fase 135)
                         aceita apenas atribuição, aritmética linear (+,-,*), comparações
                         mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), ...
```

A mensagem lista tudo que o subset aceita, sem dizer qual construto do programa
foi recusado nem onde: o span é `1:1..1:1`, a primeira linha do arquivo. Em um
programa de 1.500 linhas, isso é uma busca binária manual — foi assim que este
limite foi localizado.

Combinado com D-03 abaixo, o resultado é que **não existe hoje forma de negar um
`logica` que funcione nos dois backends**. A concha resolve sem operador nenhum:

```pink
carinho nao(b: logica) -> logica {
    talvez b {
        mimo falso;
    }
    mimo verdade;
}
```

---

# Parte 2 — defeitos

Nenhum dos quatro tinha teste, exemplo ou uso no repositório antes deste
experimento. Os três primeiros foram encontrados escrevendo a concha; o quarto,
tentando compilá-la para nativo.

## D-01 — `sempre que` com `&&` ou `||` na condição nunca termina

Repro: `limites/d01_laco_com_condicao_composta.pink`

```pink
nova muda b: bombom = 0;
sempre que b < 3 && positivo(b) {
    b = b + 1;
}
falar("terminou com", b);   // nunca chega aqui
```

O laço deveria dar três voltas. Ele não termina — nem no interpretador
(`--run`), nem no binário nativo (`build --nativo`). A condição é avaliada
**uma única vez**, na entrada.

A prova está na própria IR que a Pinker imprime:

```
$ pink --cfg-ir limites/d01_laco_com_condicao_composta.pink
    block loop_cond_0:
      %t0 = lt<bombom> %b#0, 3:bombom
      br %t0, logic_rhs_1, logic_short_2
    block logic_rhs_1:
      %t1 = call positivo(%b#0) -> logica
      let %logic#0 = %t1
      jmp logic_join_3
    block logic_short_2:
      let %logic#0 = falso:logica
      jmp logic_join_3
    block logic_join_3:
      br %logic#0, loop_0, loop_join_4
    block loop_0:
      ...
      jmp logic_join_3          <-- aresta de retorno para o FIM da condição
```

A aresta de retorno do corpo salta para `logic_join_3` — o bloco onde a
avaliação da condição **termina** — em vez de `loop_cond_0`, onde ela
**começa**. `%logic#0` guarda o valor da primeira avaliação e nunca é
recalculado.

**Origem** — `src/cfg_ir.rs`, `InstructionIR::While` (linha 700):

```rust
let (cond, cond_end_idx) = self.lower_value_operand(condition, cond_idx, *span)?;  // :710
...
self.loop_continue_stack.push(self.blocks[cond_end_idx].label.clone());            // :723
...
self.blocks[body_current].terminator =
    Some(TerminatorIR::Jump(self.blocks[cond_end_idx].label.clone()));             // :733
```

Tanto a aresta de retorno (`:733`) quanto o alvo de `continuar` (`:723`) usam
`cond_end_idx` (bloco final da condição) em vez de `cond_idx` (bloco inicial).
Para condição simples os dois são o mesmo bloco e nada aparece; para condição
com curto-circuito, não são — e o laço perde a reavaliação.

O defeito nasce na CFG IR, que é a camada compartilhada da pipeline congelada:
por isso atinge interpretador e nativo igualmente. Nenhum `sempre que` com `&&`
ou `||` existia em `examples/`, `apps/` ou `tests/` — o construto nunca tinha
sido exercitado.

**Contorno usado na concha** (cinco laços):

```pink
sempre que k < total {
    talvez nao(e_char_nome(indice_verso(linha, k))) {
        quebrar;
    }
    ...
}
```

## D-02 — literal `verso` com `\0` gera assembly inválido

Repro: `limites/d02_literal_com_nul_no_nativo.pink`

`"a\0b"` passa pelo léxico (o escape `\0` é aceito em `src/lexer.rs`), pela
semântica e pelo interpretador. O emissor `.s` escreve o byte cru dentro de
`.ascii`:

```asm
.Lpinker_verso_0:
  .quad 3
  .ascii "a<NUL>b"
```

e a montagem falha:

```
/tmp/nat/d02.s:6: Error: invalid character '"' in mnemonic
```

Quem reclama é o GNU `as`, sobre um arquivo `.s` que o usuário não escreveu, sem
span do fonte Pinker. É o único caso desta lista em que a Pinker **não** cospe o
motivo: ou o emissor escapa o byte (`\000`), ou a validação recusa o literal com
mensagem própria.

## D-03 — `logica == logica` derruba o interpretador

Repro: `limites/d03_logica_igual_logica.pink`

```
pink --check        -> aceita
pink build --nativo -> monta, roda e imprime "negado sem operador"
pink --run          -> Erro Runtime: operação inteira exige valores inteiros [instr: cmp_eq]
```

A semântica aceita a comparação; o backend nativo a implementa; a máquina do
interpretador exige inteiros no `cmp_eq`. É divergência de paridade em direção
oposta à de L-18 — e é a razão de a concha não poder usar nem `!` nem
`== falso`.

## D-04 — `criar_arquivo` tem semânticas diferentes nos dois backends

Repro: `limites/d04_criar_arquivo_diverge.pink` (rode duas vezes)

- interpretador: `fs::write(path, "")` — **trunca** e segue
  (`src/interpreter.rs:4599`);
- runtime nativo: `OpenOptions::new().write(true).create_new(true)` — **falha**
  se o arquivo existir (`runtime/pinker_rt/src/lib.rs:2206`), e a falha é fatal:

```
Erro de Execução (pinker_rt): falha ao criar arquivo '/tmp/pinker_d04.txt': File exists (os error 17)
```

Para uma shell isso é exatamente o `>` do bash: o mesmo roteiro que roda no
interpretador mata o binário nativo na segunda execução. A concha contorna
removendo o arquivo antes de criar — o que é uma corrida disfarçada, não uma
solução.

---

# Parte 3 — a Pinker cospe o motivo?

Era a pergunta do experimento. A resposta honesta, medida caso a caso:

**Cospe bem (mensagem + span + nome do construto):** L-01, L-02, L-05, L-11,
L-14. São os erros de semântica. Nome da função, aridade esperada, linha e
coluna, com o trecho do fonte apontado. Não precisa de nada.

**Cospe como valor, o que é melhor ainda:** L-06, L-08 e a leitura de arquivo.
`Resultado<T,E>` transformou "o processo morreu" em texto que o programa pode
inspecionar. Foi o que permitiu a concha reagir a `comando inexistente` em vez
de morrer junto — é a peça mais adulta da superfície atual.

**Cospe, mas não diz onde:** L-17 e L-18. A validação do backend textual emite o
catálogo do que aceita, com span `1:1..1:1`. Sabe-se que algo foi recusado; não
se sabe o quê nem onde. Nomear o callee (`ouvir_verso_ou`) e o construto
(`operador '!'`) e preservar o span da IR seria uma melhoria barata e grande.

**Não cospe nada:** L-03 (trava), L-04, L-09, L-16 (comportam-se em silêncio de
um jeito diferente do esperado) e, sobretudo, **D-01** — o laço infinito é o
pior caso possível: nenhum erro, nenhum aviso, nenhuma saída, e a mesma coisa
nos dois backends.

**Cospe em nome de outro:** D-02, onde quem reclama é o GNU `as`.

---

# Parte 4 — o que faltaria para uma shell completa

Em ordem de quanto destrava, com base no que doeu de verdade ao escrever a concha:

1. **corrigir D-01** — enquanto existir, `&&` em condição de laço é uma
   armadilha silenciosa para qualquer programa Pinker, não só para a concha;
2. **`ouvir*` no runtime nativo** (L-17) — é a única coisa entre a Pinker e uma
   shell compilada;
3. **paridade de negação** (L-18/D-03) — hoje não existe negação portátil;
4. **fluxo contínuo entre processos** (L-03) — pipeline de verdade, com o
   consumidor começando antes de o produtor terminar, e SIGPIPE chegando;
5. **escrita falível como valor** (L-13) — `criar_arquivo_resultado`,
   `escrever_verso_resultado`, na mesma forma de `ler_arquivo_resultado`;
6. **stderr como fluxo** (L-04) e **saída sem quebra de linha** (L-09) — juntas,
   viram prompt na mesma linha e diagnóstico no canal certo;
7. **convenção única de índice em `verso`** (L-12) — ou tudo codepoint, ou uma
   família explícita de bytes;
8. **ordem entre `verso`** (L-11) — `comparar_verso` acaba com a tabela manual;
9. **`mudar_diretorio`** (L-01) e **identidade do processo** (L-14);
10. **`fork`/jobs** (L-05) e **terminal para o filho** (L-07) — os mais caros, e
    os únicos que exigem decisão de design sobre o que a Pinker quer ser perto
    do sistema operacional.

Nada dessa lista é sugestão de roadmap: é o inventário do que a concha encostou
e não passou.
