# Mapa de implementação do content-machine

Data: 2026-11-18. Complementa o design (`docs/superpowers/specs/2026-11-18-cortes-locais-design.md`). Nada aqui foi construído.

## 1. Decisões de entrada

- Webcam **sempre detectada na gravação principal** (sem arquivo isolado do Source Record, para evitar um encode a mais). Source Record desligado.
- 100% automático, pós-gravação. Local. IA opcional e plugável.
- Máquina: Ryzen 5 5600X, 32 GB RAM, RTX 3060 12 GB, CUDA Toolkit 13.0 instalado, `uv` 0.10, Node 24, Ollama 0.40, FFmpeg com NVENC. **Sem** Rust nem Go instalados.

## 2. Pipeline, etapa por etapa

| # | Etapa | Entrada → saída | Como |
|---|---|---|---|
| 1 | **ingest** | MKV → `sessao` | `ffprobe`: duração, resolução, fps, faixas (Mix/Jogo/Comunicação/Mic). Hash do conteúdo (chave de cache). Gravações antigas têm 1 faixa: marca `audio=mix`. |
| 2 | **proxies** | MKV → áudio 16 kHz mono por faixa + vídeo 320x180 a 4 fps | FFmpeg com decode NVDEC (`-hwaccel cuda`). Todo o resto lê os proxies, não o 1080p60. É o que torna a análise rápida. |
| 3 | **webcam** | proxies → retângulo + confiança | Ver seção 4. |
| 4 | **sinais** | proxies → séries temporais (1 valor por 0,25 s) | Ver seção 3. Cada sinal é um módulo independente. |
| 5 | **transcrição** | áudio do Mic → segmentos com palavras | Whisper (GPU) com VAD. Só a faixa do Mic (ou Mix nas antigas). |
| 6 | **candidatos** | sinais → janelas | Normaliza (z-score robusto por sessão), soma ponderada, picos, janela com margem, não-máxima-supressão. |
| 7 | **ajuste de fronteira** | janelas + transcrição → janelas | Início e fim encaixam em silêncio/fim de frase; duração mínima e máxima. |
| 8 | **IA (opcional)** | top-N candidatos → ordem, título, legenda | Só texto + resumo dos sinais (sem vídeo). JSON validado por schema. |
| 9 | **render** | janela + retângulo → MP4 16:9 e 9:16 | FFmpeg `filter_complex`, NVENC, legendas ASS, loudnorm. |
| 10 | **manifesto** | tudo → `manifest.json` + SQLite | Rastreia por que cada corte foi escolhido. |

## 3. Sinais (o coração da qualidade)

Todos produzem uma série normalizada; o peso de cada um se calibra com o gabarito.

1. **Energia do jogo** (faixa 2): loudness curto (EBU R128 momentâneo), fluxo espectral e detecção de onset. Explosões, tiros em sequência, subida súbita.
2. **Reação do Mic** (faixa 4): pico de energia, taxa de fala, exclamações e risada. Palavras-chave vêm do Whisper. É provavelmente o sinal mais forte para "momento bom".
3. **Voz dos amigos** (faixa 3): atividade e energia da Comunicação. Reação coletiva.
4. **Movimento e mudança de cena** (vídeo proxy): diferença entre quadros e `scdet`. Distingue combate de menu/loading (penaliza menu, mapa e tela parada).
5. **Kill feed do BF6** (fase 2, opcional): OCR/template na região do kill feed. Só depois de a base funcionar.
6. **Penalidades:** silêncio longo, tela de carregamento, mapa aberto, AFK.

Fusão: `score = Σ peso_i · z_i`, com bônus quando 2+ sinais disparam juntos. Os pesos iniciais vêm de regra simples e são ajustados por busca em grade contra o gabarito (recall e precisão nos K primeiros).

## 4. Webcam inteligente (requisito obrigatório)

Entrada: proxies da gravação principal (a webcam é um overlay estático sobre o jogo).

1. **Mapa de estaticidade:** desvio padrão temporal por pixel em ~200 quadros espalhados. Overlay de webcam tem variação baixa mas não nula (é um rosto), ao contrário do HUD, que é quase constante, e do jogo, que varia muito.
2. **Detecção de rosto** (YuNet via OpenCV, ou SCRFD): rostos recorrentes na mesma região em vários quadros.
3. **Refino do retângulo:** a partir do rosto, expande até as bordas retangulares fortes (Canny + componentes conectados) e valida proporção (16:9, 4:3 ou 1:1).
4. **Confiança:** rosto presente em >70% dos quadros amostrados, retângulo estável (variação de posição < 2 px) e proporção plausível.
5. **Fallbacks:** retângulo salvo da sessão anterior parecida; senão corte só do jogo, com aviso. Nunca recorte errado.
6. **Critério de aceite:** IoU ≥ 0,9 nas 4 gameplays de teste, comparado a um retângulo marcado à mão.

Composição 9:16 (1080x1920): webcam em cima (largura total, altura proporcional), jogo embaixo recortado da **região central** do quadro (exclui o overlay da webcam e as laterais de chat/alertas), com fundo desfocado se sobrar espaço.

## 5. IA plugável

Interface única `Ranker.rank(candidatos, transcrição, resumo_sinais) -> [{id, nota, título, legenda, motivo}]`, saída validada por JSON Schema.

- **`claude` headless:** `claude -p ... --output-format json` com o JSON Schema, usando a assinatura. Isolar no adaptador; formato pode mudar entre versões.
- **API compatível com OpenAI:** URL base + chave em variável de ambiente (nunca em arquivo versionado). Cobre OpenAI e vários outros provedores.
- **Local:** Ollama (já instalado) ou llama.cpp via a mesma API compatível.
- Custo e privacidade: só texto, só os top-N candidatos (por exemplo, 30), nunca o vídeo bruto.
- Sem IA configurada, o pipeline funciona só com os sinais.

## 6. Encode e render

- Gravação do OBS: HEVC NVENC. Cortes: **H.264 NVENC** (compatibilidade máxima com redes sociais), `-preset p5`, `-cq 19` ou VBR alvo ~12 Mbps para 1080x1920, 60 fps, `-profile high`, `+faststart`.
- Áudio: `loudnorm` de duas passagens a -14 LUFS (padrão de plataformas), mixagem das faixas (jogo, comunicação, mic) com **ducking** do jogo e da comunicação quando o mic fala.
- Legendas: ASS gerado a partir das palavras do Whisper, queimadas pelo FFmpeg.
- Fila de render: uma sessão NVENC por vez para não competir com o OBS.

## 7. Dados e banco

**SQLite** (arquivo único, local, sem servidor), um banco por projeto de trabalho. Séries temporais e proxies ficam em arquivos (`.npy`/Parquet), indexados por hash no banco.

Tabelas: `sessao` (arquivo, hash, faixas, fps), `webcam` (sessão, retângulo, confiança, método), `sinal` (sessão, nome, versão, caminho do arquivo), `transcricao_segmento`, `candidato` (janela, pontuação, sinais), `corte` (candidato, formato, caminho, título), `gabarito` (sessão, início, fim, nota), `avaliacao` (execução, configuração, métricas). Postgres e DuckDB ficam fora: não há concorrência nem consulta analítica pesada que justifique.

## 8. Interfaces

- **CLI** (`cortes analyze | suggest | render | eval | label`).
- **MCP:** servidor fino sobre as mesmas funções (`suggest_clips`, `render_clip`, `get_manifest`).
- **Ferramenta de gabarito:** página local simples (HTML + vídeo em velocidade alta) para o Matheus marcar momentos.
- **Config:** um arquivo TOML (pesos, margens, formato, adaptador de IA).

## 9. Linguagem: comparação

O trabalho pesado (decodificar, codificar, Whisper) roda no FFmpeg e na GPU, em qualquer linguagem. A linguagem pesa na **velocidade de iteração**, no **ecossistema de ML** e na **distribuição**.

| | Python | Rust | Go |
|---|---|---|---|
| Whisper na GPU | `faster-whisper` pronto, com batching e timestamps por palavra | `whisper-rs` compila o whisper.cpp: exige CUDA Toolkit, Visual Studio C++/Clang, CMake e LLVM ([BUILDING.md](https://github.com/tazz4843/whisper-rs/blob/master/BUILDING.md)) | binding comunitário antigo, ou rodar `whisper.cpp` como processo externo |
| Visão (rosto, OCR) | OpenCV, ONNX Runtime, RapidOCR, YOLO: tudo pronto | `ort` + modelos ONNX: viável, mais trabalho | `gocv` exige OpenCV C++; ecossistema fraco |
| FFmpeg | processo externo | `ez-ffmpeg`/`ffmpeg-next` em processo ([visão geral](https://dev.to/yeauty/the-state-of-media-processing-in-rust-2026-what-each-crate-actually-covers-2k2c)) | processo externo (bindings fracos) |
| MCP | SDK oficial, Tier 1 | SDK oficial (`rmcp`), Tier 1 segundo fonte secundária | SDK oficial, Tier 1 ([tiers](https://modelcontextprotocol.info/community/sdk-tiers/)) |
| Iteração e avaliação (numpy, scipy, notebooks) | melhor | boa, mais lenta | fraca |
| Binário único para distribuir | não (precisa de ambiente) | sim | sim |
| Pronto nesta máquina | uv + wheels | falta instalar toolchain | falta instalar |

Verificação feita: o `uv` resolve `faster-whisper`, `ctranslate2`, `onnxruntime-gpu`, `opencv-python-headless`, `mediapipe`, `numpy`, `scipy`, `librosa`, `rapidocr-onnxruntime`, `ultralytics` e `torch` em Python 3.12, 3.13 e 3.14 para Windows. Isso foi só resolução de dependências, não instalação nem teste de GPU.

Benchmarks de Whisper (nenhum em RTX 3060; números de outras placas): faster-whisper e whisper.cpp ficam próximos sem batching; com batching o faster-whisper ganha ([comparativo](https://eurokommerz.at/blog/faster-whisper-whisperx-whisper-cpp-compared/), [teste de backends](https://www.x2q.net/post/whisper-backend-shootout/)). Todos cabem nos 12 GB da 3060. Benchmarkar na máquina antes de fixar.

### Recomendação

**Python 3.12 (gerenciado pelo `uv`) para a versão 1.** O maior risco do projeto é acertar a qualidade dos cortes, não a velocidade de execução. Isso pede iterar rápido em sinais, pesos e avaliação, onde Python é muito mais produtivo e tem tudo pronto em wheels. A performance real depende do FFmpeg e da GPU, que são as mesmas em qualquer linguagem.

**Go não ajuda:** teria que chamar FFmpeg e Whisper como processos externos e não tem ecossistema de visão, então viraria só cola.

**Rust é a melhor alternativa** se o objetivo for um binário distribuível e robusto no longo prazo. O custo é instalar a toolchain, resolver o build do whisper-rs com CUDA e iterar mais devagar nas heurísticas.

**Caminho híbrido:** as etapas se falam por arquivos (JSON, Parquet, `.npy`) e pelo SQLite. Depois que a qualidade estiver validada, qualquer etapa quente (extração de sinais, render) pode ser reescrita em Rust sem mexer no resto.

## 10. Estrutura do repositório

```
content-machine/
  pyproject.toml          # uv, Python 3.12, ruff, pytest
  src/cortes/
    ingest/ proxies/ webcam/ signals/ transcribe/
    candidates/ boundary/ ranker/ (claude, openai_compat, ollama)
    render/ db/ cli.py mcp_server.py
  tests/                  # golden files, testes dos sinais, da webcam
  eval/                   # gabarito, métricas, teste cego
  configs/default.toml
  docs/
```

## 11. Testes e avaliação

- **Unitários:** cada sinal com áudio/vídeo sintético (tom, clique, quadro estático).
- **Golden:** webcam (IoU contra o retângulo marcado), fronteiras de frase, comandos FFmpeg gerados.
- **Avaliação:** gabarito nas 4 gameplays; métricas recall e precisão@K; ablação por sinal; teste cego contra o OpenShorts.
- **Regressão:** cada mudança de pesos roda `cortes eval` e registra no SQLite.

## 12. Riscos e pontos em aberto

- O gabarito depende do Matheus. Sem ele não há medida.
- Gravações antigas têm só o mix de áudio: sinais de jogo e voz ficam misturados nelas.
- A detecção de webcam deve cobrir overlay parcialmente translúcido ou com forma não retangular; se aparecer, tratar como caso de teste.
- Ponderação dos sinais e limiares: calibrar com dados, não por palpite.
- `claude -p` pode mudar de formato; adaptador isolado com teste de contrato.
- Whisper: escolher modelo (large-v3-turbo vs medium) por benchmark na 3060.
- Parâmetros NVENC (B-frames, AQ psicovisual) alterados no OBS: validar com uma gravação real.

## 13. Ordem de construção sugerida

1. ingest + proxies + SQLite.
2. **Webcam** (requisito obrigatório) com teste IoU nas 4 gameplays.
3. Render 16:9 e 9:16 com um retângulo fixo (valida composição e encode).
4. Sinais de áudio e movimento + candidatos + `eval` com o gabarito.
5. Whisper + fronteiras de frase + legendas.
6. Adaptadores de IA + ranker.
7. MCP + ferramenta de gabarito.
8. Teste cego contra OpenShorts; calibração final.
9. Fase 2: kill feed do BF6.
