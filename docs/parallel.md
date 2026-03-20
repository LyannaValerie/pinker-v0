# parallel.md — A fantasia orientadora da Pinker

> **Nota de precedência**: `docs/roadmap.md` define o que vem agora.
> `docs/future.md` organiza o que o projeto provavelmente vai precisar.
> `docs/parallel.md` guarda a fantasia orientadora, a alma e a ambição conceitual da Pinker.
> Nenhum item deste documento cria fase funcional, altera o roadmap ativo ou transforma sonho em promessa técnica.

---

## 1. O que este arquivo é

Este é o documento visionário da Pinker.

Não é um backlog. Não é um roadmap. Não é uma lista de tarefas.
É o lugar onde a Pinker existe inteiramente, antes de qualquer restrição de implementação.

Aqui vivem as ideias que ainda não sabemos realizar, as que talvez só se tornem viáveis muito no futuro, e as que são mais sobre identidade do que sobre feature. Aqui está o que o projeto quer ser, não apenas o que ele faz hoje.

`parallel.md` deve ser consultado quando houver dúvida estratégica sobre a direção do projeto: quando um bloco do roadmap for concluído e for preciso orientar o próximo passo com algo além do pragmatismo; quando uma decisão de design parecer oposta ao espírito da Pinker; quando o projeto estiver tecnicamente correto mas estiritamente correto mas estiritamente vazio.

---

## 2. Como usar este arquivo

Leia-o antes de tomar decisões de design que não estejam cobertas pelo roadmap.
Consulte-o quando precisar sentir a direção, não apenas calcular o próximo passo.
Use-o como contraponto quando houver risco de o projeto virar mais uma ferramenta sem personalidade.

Não use este arquivo para:
- justificar desvio do roadmap ativo;
- criar fase funcional não prevista;
- argumentar que algum item aqui deve ser implementado antes do Bloco atual;
- transformar sonhos em compromisso técnico.

A regra de ouro é simples:
- o roadmap decide o agora;
- o future organiza o provável;
- o parallel protege o sonho.

---

## 3. O que este arquivo não é

Não é um segundo `future.md`. O `future.md` é inventário técnico estruturado por camadas de dependência, com itens que o projeto provavelmente vai precisar. O `parallel.md` é outra coisa: é o espaço onde a Pinker existe como intenção, como identidade, como fantasia orientadora.

Não é um manifesto caótico. Este documento tem tom sério e controlado. Não é lugar de excesso retórico nem de promessas infundadas. É um documento de direção profunda.

Não substitui nenhum documento existente. `roadmap.md`, `future.md`, `phases.md`, `agent_state.md` e `handoff_codex.md` continuam sendo os documentos operacionais e continuam tendo precedência sobre este arquivo em tudo que diz respeito à execução real.

---

## 4. Tese central da Pinker

A Pinker não é apenas mais uma linguagem de programação com palavras em português.

Ela é um projeto de soberania sobre a máquina. A aposta é que é possível criar uma linguagem de sistemas que seja ao mesmo tempo rigorosa, capaz de programar hardware diretamente, e íntima — no vocabulário, no tom, na relação com quem escreve nela.

A maioria das linguagens de sistemas fala com você como um documento técnico. A Pinker quer falar como alguém que sabe o que está fazendo e não precisa intimidar para provar isso.

Esse equilíbrio — rigor técnico sem frieza, proximidade sem infantilidade — é a aposta central do projeto. Ele aparece no vocabulário (`sussurro` para inline asm, `ninho` para struct, `fragil` para volatile), na escolha de cada keyword, e deve aparecer também no que a linguagem diz quando algo dá errado.

---

## 5. Rosa

O nome "Pinker" não é acidental. Rosa é a cor do projeto, não apenas como estética mas como posicionamento.

Em programação de sistemas, o padrão visual e tonal é o da seriedade árida: cinza, preto, verde terminal, nomes técnicos, diagnósticos impessoais. A Pinker aposta no contrário — não porque quer ser decorativa, mas porque acredita que a identidade de uma linguagem importa tanto quanto sua capacidade técnica.

Rosa aqui não é suavidade. É afirmação de que é possível ser ao mesmo tempo capaz e acolhedor. É a recusa de que linguagens de sistemas precisem ser frias para serem levadas a sério.

A cor do projeto é um posicionamento: a Pinker é firme onde precisa ser, e próxima onde pode ser.

---

## 6. A linguagem como entidade viva

A Pinker aspira ser uma linguagem com voz. Não no sentido metafórico de "ter estilo de código" — mas no sentido de que quando ela fala com você, você sente que há alguém do outro lado.

Isso significa:

**Diagnósticos com personalidade.** Erros de compilação que não apenas descrevem o problema, mas o contextualizam com a perspectiva da linguagem. Não `type mismatch: expected u8, got bombom`, mas algo que diz o que foi tentado, por que não funciona, e o que fazer a seguir — com brevidade e sem crueldade.

**Tom consistente.** Mensagens de erro, avisos, sugestões: tudo deve soar como a mesma voz. Uma voz que conhece a situação melhor do que o programador no momento do erro, e que está do lado dele.

**Sarcasmo com medida.** Há situações onde um erro pode ser respondido com ironia leve — não para humilhar, mas para marcar que certa combinação é particularmente improvável ou que o programador claramente está num momento de confusão. O sarcasmo deve ser opcional, configurável, e sempre gentil. Nunca cruel.

**Proteção como cuidado.** A Pinker tem `fragil` para volatile, `sempre que` para while, `quebrar` para break. Cada nome reflete uma postura: o compilador não é fiscal, é parceiro. Quando ele rejeita algo, é porque sabe algo que você talvez tenha esquecido.

---

## 7. Semântica de pergunta `?`

A Pinker sonha com um operador `?` que seja mais do que propagação de erro no estilo Rust.

A ideia é que `?` seja uma forma de o programador admitir incerteza ao compilador — e o compilador ser capaz de responder a essa incerteza de forma estruturada. Não apenas "propague o erro se houver", mas "esta é uma zona onde eu não tenho certeza do que vai acontecer; ajude-me a pensar sobre isso".

Isso pode se manifestar como:
- operador de propagação de resultado com diagnóstico contextual melhor do que o padrão;
- forma de marcar um bloco ou expressão como "incerto" e receber do compilador uma análise dos casos possíveis;
- sintaxe para perguntas ao compilador sobre o estado do tipo em um ponto específico.

A semântica exata ainda não está definida. Mas a intenção está: `?` não é só conveniência de ergonomia — é a Pinker reconhecendo que há situações onde o programador precisa de orientação, não apenas de validação.

---

## 8. Sintaxe dual: textual e simbólica

A Pinker imagina que no longo prazo sua sintaxe seja genuinamente dual: você pode escrever qualquer programa de duas formas equivalentes.

**Forma textual**: `carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }` — palavras em português, legível em voz alta, próxima da língua natural.

**Forma simbólica**: a mesma estrutura expressa com símbolos, para contextos onde o programador prefere densidade visual.

As duas formas são 100% equivalentes, convertem uma para a outra sem perda, e o programador pode misturá-las dentro do mesmo arquivo se quiser.

A motivação não é estética: é que diferentes pessoas pensam de formas diferentes, e uma linguagem que respeita isso pode ser usada de formas mais ricas. A forma textual convida o programador iniciante e o contexto pedagógico. A forma simbólica serve o especialista que quer densidade expressiva.

Nenhuma das duas é "mais séria" que a outra.

---

## 9. Terminal próprio

A Pinker sonha com um terminal que fale a mesma língua que ela.

Não um shell genérico. Um ambiente interativo onde os conceitos da linguagem — `carinho`, `eterno`, `ninho`, `seta` — são cidadãos de primeira classe. Onde você pode inspecionar o estado de um programa Pinker com os mesmos termos que usou para escrevê-lo.

Isso significa:
- diagnósticos inline com a mesma voz do compilador;
- inspeção de tipos com a nomenclatura da linguagem;
- execução interativa com feedback em português;
- histórico que entende o contexto semântico, não apenas os comandos.

O terminal próprio não é sobre ergonomia de shell. É sobre soberania: poder usar a Pinker dentro de um ambiente que foi construído para ela, não adaptado para ela.

---

## 10. Partes do ecossistema escritas em Pinker

O objetivo de longo prazo mais ambicioso do projeto é que partes significativas do próprio ecossistema da Pinker sejam escritas em Pinker.

Isso inclui:
- o compilador, eventualmente (self-hosting);
- o package manager;
- o terminal;
- ferramentas de análise e diagnóstico;
- partes da biblioteca padrão.

Esse nível de self-hosting não é apenas um marco técnico. É a comprovação de que a linguagem é capaz o suficiente para ser sua própria infraestrutura. É o momento em que a Pinker deixa de ser um projeto sobre uma linguagem e se torna uma linguagem que se usa para fazer coisas reais — inclusive a si mesma.

O caminho até lá é longo e passa pelo roadmap técnico. Mas a direção importa: cada decisão de design do compilador deve considerar se ela facilita ou dificulta o dia em que o compilador será reescrito na própria linguagem.

---

## 11. Soberania total: self-hosting, terminal, ecossistema soberano

A Pinker é um projeto de soberania sobre a máquina — e soberania de verdade significa não depender de nada que você não entende ou não controla.

O sonho final é que um programador usando a Pinker possa, se quiser:
- escrever código que roda diretamente no hardware, sem camadas invisíveis;
- usar ferramentas construídas na própria linguagem;
- entender completamente o caminho do código-fonte até a execução;
- operar num ambiente onde cada palavra tem significado próprio e conhecido.

Isso é diferente de "reinventar a roda". É sobre oferecer um caminho alternativo para quem quer operar com plena consciência do que está acontecendo na máquina — sem ter que aprender cinco linguagens intermediárias para chegar lá.

A soberania não é isolamento. A Pinker pode interoperar com C, pode chamar syscalls, pode funcionar em hardware existente. Mas pode também existir como ecossistema independente para quem quiser esse caminho.

---

## 12. Regra de ouro

Ao trabalhar neste projeto, quando houver dúvida estratégica sobre direção:

**O roadmap decide o agora.**
O que está em `docs/roadmap.md` define o próximo passo. Não há razão para antecipar frentes horizontais enquanto o bloco ativo tiver itens abertos.

**O future organiza o provável.**
O que está em `docs/future.md` é o inventário técnico estruturado do que o projeto vai precisar. É o backlog honesto, organizado por dependências reais.

**O parallel protege o sonho.**
O que está aqui não precisa ser implementável hoje. Não precisa estar no roadmap. Não precisa competir com nada. Está aqui para que, quando o projeto estiver tecnicamente avançado o suficiente, saibamos em que direção estávamos apontando desde o começo.

A disciplina técnica e a fantasia orientadora não são opostos. O roadmap existe para que o parallel não seja só sonho. O parallel existe para que o roadmap não seja só trabalho.

---

*Este arquivo é estável. Pode ser expandido, mas não deve ser reescrito com frequência. É um documento de identidade, não de planejamento.*
