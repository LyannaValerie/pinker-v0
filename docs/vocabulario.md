# Vocabulario Pinker

Catalogo vivo de keywords fofinhas para a linguagem Pinker.
Palavras em portugues, carinhosas, sem diminutivo.
Consultar antes de implementar uma feature nova — riscar ao implementar.

---

## Keywords atuais (implementadas)

| Keyword        | Conceito         | Espirito              |
|----------------|------------------|-----------------------|
| `pacote`       | package          | organizacao           |
| `carinho`      | function         | afeto, cuidado        |
| `mimo`         | return           | presente, oferenda    |
| `talvez`       | if               | possibilidade         |
| `senao`        | else             | alternativa           |
| `sempre que`   | while            | persistencia          |
| `quebrar`      | break            | ruptura gentil        |
| `continuar`    | continue         | seguir em frente      |
| `eterno`       | const global     | permanencia           |
| `nova`         | let (declaracao) | nascimento            |
| `mut`          | mutable          | mudanca               |
| `bombom`       | integer (u64)    | doce                  |
| `logica`       | boolean          | razao                 |
| `verdade`      | true             | certeza               |
| `falso`        | false            | negacao               |
| `principal`    | main/entry       | protagonista          |
| `apelido`      | type alias       | nome alternativo      |
| `ninho`        | struct           | agrupamento acolhedor |
| `virar`        | cast explícito   | transformação visível |
| `peso`         | sizeof explícito | medida estática       |
| `alinhamento`  | alignof explícito| alinhar com cuidado   |
| `fragil`       | volatile         | cuidado redobrado     |

---

## Sugestoes para features futuras

Cada entrada marca: conceito, keyword sugerida, alternativas, e o sentimento.
Ao implementar, mover para a tabela de cima e riscar daqui.

### Tipos novos

| Conceito         | Sugestao    | Alternativas        | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| float            | `bolha`     | `brisa`, `pluma`     | leve, flutuante                     |
| string / texto   | `verso`     | `prosa`, `fala`      | palavras como poesia                |
| byte (u8)        | `migalha`   | `grao`               | pequeno mas precioso                |
| char             | `letra`     | `sinal`              | simples, direto                     |
| void explito     | `nulo`      | `vazio`              | ausencia serena                     |
| ponteiro / ref   | `seta`      | `olhar`, `aceno`     | direcao, apontar                    |

### Estruturas de dados

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| ~~struct~~       | ~~`ninho`~~ | `cesta`, `colcha`    | implementado na Fase 47             |
| enum             | `leque`     | `flor`, `arco`       | variedade, abrir opcoes             |
| array            | `colar`     | `fila`, `cordao`     | sequencia encadeada                 |
| tuple            | `par`       | `laco`, `broche`     | coisas juntas                       |
| map / dicionario | `almanaque` | `cofre`, `gaveta`    | guardar com organizacao             |

### Controle de fluxo

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| for / iteracao   | `passeio`   | `ronda`, `caminho`   | caminhar por cada elemento          |
| match / switch   | `encaixe`   | `espelho`, `reflexo` | encontrar o lugar certo             |
| loop infinito    | `roda`      | `gira`, `ciranda`    | movimento sem fim                   |
| try / catch      | `amparo`    | `abrigo`, `resguardo`| protecao contra tropecos            |
| throw / error    | `tropeco`   | `susto`              | algo inesperado, mas superavel      |

### Visibilidade e modulos

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| pub / publico    | `aberto`    | `livre`              | transparencia                       |
| private          | `segredo`   | `intimo`             | protecao, guardado                  |
| import / use     | `trazer`    | `buscar`, `chamar`   | convidar para perto                 |
| module           | `canto`     | `ala`, `sala`        | um espaco proprio                   |
| self             | `eu`        |                      | identidade                          |

### Orientacao a tipos

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| trait/interface  | `molde`     | `jeitinho`, `feitio` | forma de ser                        |
| impl             | `vestir`    | `assumir`            | vestir um molde, incorporar         |
| ~~type alias~~   | ~~`apelido`~~ | `alcunha`          | implementado na Fase 45             |
| ~~cast / as~~    | ~~`virar`~~ | `tornar`             | implementado na Fase 50             |
| ~~sizeof~~       | ~~`peso`~~  | `tamanho`            | implementado na Fase 51             |
| generic          | `qualquer`  | `molde livre`        | flexibilidade                       |

### Programacao de sistemas

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| unsafe           | `coragem`   | `ousadia`            | saber que ha risco e seguir         |
| ~~volatile~~     | ~~`fragil`~~| `instavel`           | implementado na Fase 52             |
| ~~align/alinhamento~~ | ~~`alinhamento`~~ | `ajuste`     | implementado na Fase 51             |
| static           | `raiz`      | `ancora`             | fixo, inamovivel                    |
| inline asm       | `sussurro`  | `sopro`              | falar direto com a maquina, baixinho|
| alloc            | `reserva`   | `guardar`            | separar espaco com intencao         |
| free / drop      | `soltar`    | `liberar`, `largar`  | deixar ir                           |

### Concorrencia (futuro distante)

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| async            | `sonho`     | `promessa`           | algo que vai acontecer              |
| await            | `espera`    | `aguardo`            | paciencia                           |
| spawn / thread   | `broto`     | `semente`, `muda`    | algo que nasce e cresce em paralelo |
| mutex / lock     | `tranca`    | `chave`, `cadeado`   | protecao de acesso                  |
| channel          | `ponte`     | `tunel`, `passagem`  | conexao entre partes                |

### I/O

| Conceito         | Sugestao    | Alternativas         | Sentimento                          |
|------------------|-------------|----------------------|-------------------------------------|
| print            | `falar`     | `contar`, `dizer`    | expressar para o mundo              |
| read / input     | `ouvir`     | `escutar`            | receber do mundo                    |
| file open        | `abrir`     |                      | literal e acolhedor                 |
| file close       | `fechar`    |                      | encerrar com cuidado                |
| write to file    | `escrever`  | `registrar`          | deixar marca                        |

---

## Principios do vocabulario

1. **Sem diminutivo** — `ninho`, nao `ninhozinho`
2. **Portugues natural** — palavras que existem e fazem sentido sozinhas
3. **Sentimento claro** — cada keyword evoca algo afetuoso ou poetico
4. **Sem ambiguidade tecnica** — a palavra deve ser intuitiva para quem le codigo
5. **Consistencia** — manter o tom da Pinker: gentil, acolhedor, firme

---

## Como usar este documento

- Antes de implementar uma feature, consultar a sugestao aqui
- Se a sugestao nao servir, anotar a alternativa escolhida
- Ao implementar, mover a linha para "Keywords atuais"
- Novas ideias podem ser adicionadas a qualquer momento
- Se uma keyword for rejeitada, marcar com ~~riscado~~ e anotar o motivo
