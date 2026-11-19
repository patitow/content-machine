# Roadmap

Atualizado em 2026-11-18. Fonte de verdade dos requisitos: `docs/superpowers/specs/2026-11-18-cortes-locais-design.md`. Arquitetura: `docs/adr/0001-arquitetura-e-linguagem.md`.

## Feito

### Plano 1: fundação e portões
Workspace Rust, tipos de gravação, `probe` via ffprobe, fingerprint blake3 sem ler o arquivo inteiro, SQLite com migrações, ingest em uma passagem com cache atômico, CLI `latest` e `ingest`.
Portões: **S2** (ingest real: 15,25 GB em 119,8 s) e **S1** (ort + YuNet detecta webcam em gameplays reais). Plano: `docs/superpowers/plans/2026-11-18-fundacao-e-portoes.md`.

### Plano 2: webcam inteligente e render 9:16
Detector de rosto YuNet, detector de webcam (moldura nítida e recorte da pessoa), cadeia de decisão com *override* manual, retângulo salvo e corte só do jogo como último recurso, `plan_layout` + `render_9x16` com NVENC e *fallback* x264, comandos `webcam` e `render-sample`. Revisado pelo Matheus em artefato com notas; ajuste de folga acima da cabeça aplicado.
Plano: `docs/superpowers/plans/2026-11-19-webcam-e-render-9x16.md`. Resultados medidos no ADR, seção "Portões".

## Próximo

### Plano 3: sinais, candidatos e avaliação
Foco escolhido: **seleção de momentos por sinais de áudio**.

1. **Gabarito humano.** Página local onde o Matheus marca os melhores momentos (15–25 por sessão) clicando numa linha do tempo com a curva de áudio e quadros. Sem isso não há medida de qualidade; é o gargalo humano do projeto.
2. **Sinais** (cada um um módulo independente, série normalizada a 0,25 s): energia do jogo (loudness curto, fluxo espectral, onsets), fala/comunicação, movimento do vídeo.
3. **Candidatos:** z-score robusto por sessão, soma ponderada, picos, janela com margem, supressão de não-máximos.
4. **`cortes eval`:** recall e precisão nos K primeiros cortes contra o gabarito, com tolerância de janela.
5. **Ablação** do ganho de cada sinal e **teste cego** contra o OpenShorts nas mesmas gravações.

## Depois

| Plano | Entrega |
|---|---|
| 4 | Worker Whisper (GPU, VAD) para transcrição e ajuste de fronteira em fim de frase |
| 5 | Daemon + API HTTP (axum) + servidor MCP (`rmcp`) + skill `gerar-cortes`; adaptadores de IA (`claude -p`, API OpenAI-compatível, Ollama) |
| 6 | UI React + Vite + TypeScript em Electron, também no navegador e operável por agente (`aria-label`, `data-testid`) |
| Acabamento | Legendas ASS, normalização de volume, frases de abertura |
| Mais tarde | Kill feed de Battlefield 6, LivePix e Twitch |

## Débitos conhecidos

- Cache de webcam no SQLite não é invalidado quando o algoritmo muda (precisa de `--redetect` ou versão do detector).
- Em webcam quadrada, o corte vertical mantém 30% do excesso em cima em vez de centrar no rosto.
- O cache da webcam não guarda o tipo (moldura ou recorte).
- Dois ingests simultâneos do mesmo arquivo compartilham o mesmo diretório `.tmp`.
- O gabarito de julho foi lido visualmente e o retângulo anatômico foi calibrado nos mesmos 2 vídeos: risco de sobreajuste até haver mais sessões.

## Pendências de setup (não são código)

- Gravar 10 s no OBS para validar as 4 faixas de áudio e os encoders.
- Apontar a captura de jogo do OBS para o jogo a cada gravação.
- Definir o dispositivo de reprodução padrão do Windows para o fone do jogo (a faixa "Jogo" segue o padrão do Windows).
