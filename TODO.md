# TODO

O que ainda falta para a validação completa do Curie-DAQ com a interface PHYWE Cobra real.
Tudo o que se podia fazer sem o hardware físico está feito (timing, fits, gravação e runs guardados
entre sessões, porta série, lints em todo o workspace, binários Windows de 64 e 32 bits, docs).

## 1. Validação com a Cobra real

- [ ] **Confirmar o protocolo.** Foi recuperado do `RADIO2-E.EXE` e testado só contra uma Cobra
      simulada. Num terminal (`picocom -b 9600 /dev/ttyS1`, Enter = CR):
  - [ ] `X 20` responde `*`.
  - [ ] `Z 1 15 16960` (porta de 1 s) responde `*`.
  - [ ] `z 2` repetido: qual de `X` / `Y` aparece enquanto conta e se no fim vêm 8 dígitos hex.
        Se for ao contrário do esperado, nada muda no código (ambos são tratados como "ainda não").
  - [ ] O tubo GM está mesmo no contador `2`. Se não estiver, mudar `COBRA_COUNTER_CHANNEL`
        em `crates/curie-core/src/hardware/phywe.rs`.
  - [ ] Modo de flanco `1` conta os impulsos do GM. Se não contar, testar `Z 2 …` e ajustar
        `CobraEdge::Mode1` em `serial.rs`.
  - [ ] Se a Cobra faz eco dos comandos (o código ignora ecos, mas convém saber).
  - [ ] É preciso reenviar `Z` antes de cada porta. O código reenvia sempre; confirmar que
        isso não perde contagens nem dá erro.
  - [ ] A 19200 baud: `X 20` seguido de `X 21`.
- [ ] **Arranjar o `RADIO1-E.EXE`.** Na pasta copiada só estão o overlay e a ajuda do RADIO1. Confirmar
      que usa os mesmos comandos que o RADIO2 (procurar `Z `, `z 2`, `w 2` no binário).
- [ ] **Validar lado a lado com o RADIO1** na mesma sessão: mesma fonte, mesmo Δt (p.ex. 10 s × 100).
      As médias de Nc têm de coincidir dentro de σ/√n, e o fundo também.
- [ ] **Identificação da porta série:** testar portas série nativas e adaptadores USB-série (`/dev/ttyS1`,
      `/dev/ttyUSB0`, `COM1`, `COM2`) e garantir permissões (grupo `dialout` em Linux).
- [ ] **Windows:** testar o `.exe` num PC Windows com a Cobra (incluindo o binário de 32 bits,
      `target/i686-win7-windows-gnu/release/curie-ui.exe`, para PCs antigos com Windows 7). DTR e RTS
      ficam ativos, como no Linux; confirmar que a Cobra não precisa de outro estado.
- [ ] Medir o intervalo morto entre portas (leitura + novo `Z`). O eixo do tempo usa o início real
      de cada porta, por isso não enviesa o decaimento.

