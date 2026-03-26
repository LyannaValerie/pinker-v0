# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 126 — `quebrar` / `continuar` (camada 1 conservadora) no backend nativo externo**.
- Rodada funcional mínima de abertura do item 10.3 do Bloco 10.

## 2. Resultado operacional da rodada
- Bloco 10 permanece como trilha ativa.
- Itens 10.1 e 10.2 foram preservados (`u32`/`u64` em parâmetros/locais e comparações mínimas `==`, `!=`, `<`, `>`, `<=`, `>=`) e o item 10.3 foi aberto em camada 1 conservadora com recorte mínimo de `quebrar`/`continuar` em `sempre que`.
- Subset anterior (Fases 111–119) foi preservado sem regressão.
- Ordem interna canônica permanece a mesma: 10.1 inteiros mais largos; 10.2 comparações ampliadas; 10.3 `quebrar`/`continuar`; 10.4 `ninho`/heterogêneo mínimo; 10.5 `virar`; 10.6 `verso` condicional.

## 3. Continuidade preservada
- Fase funcional atual: **126**.
- Fase funcional anterior: **125**.
- Rodada documental mais recente permanece **Doc-21**.

## 4. Próximo passo correto
- Próxima rodada normal: continuar o item **10.3** em degrau pequeno seguinte, sem abrir 10.4 junto.
- Não pular ordem interna do Bloco 10.
- Não inverter `ninho` e `virar`.
- Não antecipar `verso`; item final e condicional.

## 5. Restrições explícitas do bloco
- Sem backend nativo pleno.
- Sem trilha de performance/otimizador como foco principal.
- Sem runtime grande, ABI ampla/plena, sistema geral de texto ou compostos avançados.
- Sem abertura simultânea de muitos tipos/semânticas para acelerar fechamento artificial do bloco.
