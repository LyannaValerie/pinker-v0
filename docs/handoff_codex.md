# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fases 190–206 — ergonomia, expressividade e expansão de coleções**.
- Leitura operacional canônica: expansão funcional ampla cobrindo ergonomia de linguagem (Fases 190–202) e novos tipos de coleção (Fases 203–206), fora do escopo formal do Bloco 18 mas compatível com o domínio provisório `colecao`.

## 2. Resultado operacional da rodada
- `src/lexer.rs`: comentários de bloco (`/* */`), sequências de escape em verso (`\n`, `\t`, `\\`, `\"`), strings multiline.
- `src/parser.rs`: operadores compostos de atribuição (`+=`, `-=`, `*=`, `/=`, `%=`), `repetir ... até`, `para ... de ... até`, operador ternário (`? :`), `escolha/caso/padrao`, retorno implícito, interpolação de verso (`"texto {expr} texto"`), desugaring de `para cada` para novos tipos de coleção.
- `src/semantic.rs`: novas intrínsecas utilitárias (`verso_para_bombom`, `bombom_para_verso`, `dormir`, `afirmar`, `copiar_arquivo`, `renomear_arquivo`, `aleatorio_entre`), suporte semântico completo para `lista<verso>`, `mapa<verso,verso>`, `mapa<bombom,bombom>`, `mapa<bombom,verso>` e intrínsecas de iteração associadas, `eterno` para verso.
- `src/ir.rs`, `src/ir_validate.rs`, `src/cfg_ir_validate.rs`, `src/instr_select_validate.rs`, `src/abstract_machine_validate.rs`, `src/layout.rs`: novos tipos `ListVerso`, `MapVersoVerso`, `MapBombomBombom`, `MapBombomVerso` e function sigs correspondentes em todos os estágios de validação.
- `src/interpreter.rs`: implementação runtime de todas as novas intrínsecas e tipos de coleção, incluindo iteradores internos para `para cada`.
- `src/main.rs`, `src/repl.rs`, `src/ast.rs`: suporte aos novos tipos em display e AST.
- Exemplos versionados: `examples/fase190_*` a `examples/fase206_*` (17 exemplos).
- `make ci` passa integralmente.

## 3. Próximo passo correto
- Próximo passo provável: continuar avançando funcionalidades de alta dificuldade restantes ou retomar o eixo 18.6/18.7 do Bloco 18.
- O **Bloco 18** segue como bloco oficialmente ativo.

## 4. Restrições explícitas
- Sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação continua monomorphizada.
- Sem coleções heterogêneas.
- Sem reorganização funcional ampla do engine.
- Compatibilidade global legada preservada integralmente.
