# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 115 — ABI mínima mais larga (camada 1 conservadora) no backend nativo externo**.
- Quinta fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável ampliou o limite de call direta de **até 2** para **até 3 argumentos `bombom`** no recorte Linux x86_64 hospedado.
- Convenção concreta desta camada: `%rdi` (arg0), `%rsi` (arg1), `%rdx` (arg2), `%rax` (retorno) e `%r10` temporário volátil já existente.
- Validações mínimas no caminho externo foram ampliadas: recusa explícita de função/call com **4+ argumentos** e manutenção das recusas de tipos/recursos fora do subset.
- Exemplo versionado da fase adicionado (`examples/fase115_abi_minima_mais_larga_camada1_valida.pink`) e cobertura de testes ampliada para emissão auditável do terceiro argumento.

## 3. Continuidade preservada
- Fase funcional atual passa para **115**.
- Fase funcional anterior passa para **114**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Continuar o item **9.5** apenas se necessário, em camada conservadora adicional, sem antecipar o item **9.6**.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
