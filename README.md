<div align="center">

# 🎬 content-machine

**Cortes automáticos de gameplay, 100% locais.**
Grave no OBS. Os cortes 16:9 e 9:16 saem sozinhos, depois.

![Rust](https://img.shields.io/badge/Rust-1.100-orange?logo=rust)
![FFmpeg](https://img.shields.io/badge/FFmpeg-NVENC-green?logo=ffmpeg)
![Local](https://img.shields.io/badge/100%25-local-blue)
![License](https://img.shields.io/badge/license-MIT-lightgrey)
![Status](https://img.shields.io/badge/status-em%20constru%C3%A7%C3%A3o-yellow)

</div>

---

## A ideia

Um pipeline **local e controlado por você** que pega as gravações do OBS (gameplay com webcam, ou vídeo falado) e produz os melhores momentos em **16:9** e **9:16**, sem apertar botão para marcar nada durante a gravação.

- **Tudo automático, pós-gravação.** Sem atalho de marcador, sem edição manual.
- **Local.** Seus vídeos não saem da máquina.
- **IA plugável e opcional.** `claude -p` headless, qualquer API compatível com OpenAI, ou Ollama local. O caminho básico não depende de IA; ela só reordena e escreve texto.
- **Qualidade primeiro.** Cada etapa é medida contra um gabarito feito à mão (recall e precisão), não por impressão.
- **Webcam inteligente.** A webcam é detectada na própria gravação (sem arquivo separado) e o 9:16 sai com a webcam em cima e o jogo embaixo, sem nunca cortar errado.

> Uso principal pretendido: pedir ao Claude Code *"gera os cortes do meu último vídeo"* (MCP + skill `gerar-cortes`). A UI e a CLI vêm do mesmo núcleo.

## Como funciona

```text
MKV do OBS
   │
   ├─ ingest ──────── ffprobe + 1 passagem de FFmpeg: áudio por faixa + proxy de vídeo (cache atômico)
   ├─ webcam ──────── rosto (YuNet/ONNX) + bordas persistentes → retângulo da webcam
   ├─ sinais ──────── energia do jogo, fala, movimento…            (plano 3)
   ├─ candidatos ──── picos, janelas, fronteiras em silêncio       (plano 3)
   ├─ transcrição ─── Whisper em worker                            (plano 4)
   ├─ IA opcional ─── ordem, título, legenda                       (plano 5)
   └─ render ──────── FFmpeg + NVENC → 16:9 e 9:16
```

**Decisão da webcam** (nunca um recorte errado):

1. Retângulo manual no `config.toml`
2. Detecção com confiança ≥ 0,6
3. Retângulo salvo de uma sessão parecida, se um rosto cair dentro em ≥ 60% dos quadros
4. Nada achado → corte só do jogo, com aviso

## Status

| # | Plano | Estado |
|---|---|---|
| 1 | Fundação: ingest, cache, SQLite, CLI, portões S1/S2 | ✅ feito |
| 2 | Webcam inteligente + render 9:16 | ✅ feito |
| 3 | Sinais de áudio, candidatos e avaliação (`cortes eval`) | 🔜 próximo |
| 4 | Worker Whisper | ⏳ |
| 5 | Daemon + API + MCP + skill `gerar-cortes` | ⏳ |
| 6 | UI React + Electron | ⏳ |

Detalhes e backlog em [`docs/ROADMAP.md`](docs/ROADMAP.md).

### Números medidos (RTX 3060, HDD a ~97 MB/s)

- Ingest real: **15,25 GB em 119,8 s**.
- Webcam, sessões de novembro (moldura nítida): IoU **0,976**.
- Webcam, sessões de julho (pessoa recortada, sem moldura): IoU **0,91–0,96** e contenção **≥ 0,975**.
- 9:16 1080×1920 com NVENC (`p5`, `cq 21`, teto de 14 Mbps), com *fallback* automático para x264.

## Uso

Requisitos: Windows, Rust (versão fixa em `rust-toolchain.toml`), FFmpeg/FFprobe no `PATH`, GPU NVIDIA (opcional, há *fallback* em CPU).

```powershell
cargo build --release

cortes latest                       # gravação mais recente da pasta do OBS
cortes ingest latest                # áudio + proxy de vídeo, em cache
cortes webcam latest                # retângulo da webcam e confiança
cortes render-sample latest --start 60 --dur 10   # trecho 9:16 de teste
```

Configuração em `%APPDATA%\content-machine\config.toml` (ou `CORTES_CONFIG`):

```toml
obs_dir   = 'E:\.VIDEOS GRAVADOS\OBS'
cache_dir = 'E:\.VIDEOS GRAVADOS\content-machine-cache'

# Forçar o retângulo da webcam de uma gravação (chave = nome do arquivo sem extensão)
[webcam_overrides."2025-11-30 13-54-24"]
x = 0
y = 438
w = 408
h = 284
```

O modelo de rosto fica em `models/` (YuNet, licença MIT, do OpenCV Zoo).

## Arquitetura

| Crate | Papel |
|---|---|
| `cm-domain` | Tipos puros: gravação, faixas, `Rect`, detecção de webcam |
| `cm-media` | FFmpeg/FFprobe por processo externo, ingest com cache, amostragem de quadros, render 9:16 |
| `cm-store` | SQLite (WAL) com migrações versionadas; retângulos de webcam salvos |
| `cm-vision` | Detector de rosto (ONNX Runtime) e detector de webcam |
| `cm-cli` | Binário `cortes` |

Por que Rust, FFmpeg como processo, workers de ML separados e UI em React dentro de Electron (que também roda no navegador, para um agente operar): [`docs/adr/0001-arquitetura-e-linguagem.md`](docs/adr/0001-arquitetura-e-linguagem.md), com benchmarks reais.

## Desenvolvimento

```powershell
powershell -File scripts/check.ps1   # fmt --check, clippy -D warnings, testes
cargo test -p cm-vision --test webcam_real -- --ignored --nocapture   # gate em vídeos reais (precisa dos MKVs)
```

Regras do projeto: tipos e assinaturas antes da implementação, **testes antes do código (TDD)**, código em inglês, textos para o usuário em português, nenhum caminho pede entrada interativa.

## Documentação de planejamento

Tudo que foi pensado e decidido está no repositório:

- [`docs/superpowers/specs/`](docs/superpowers/specs/): design e requisitos
- [`docs/research/`](docs/research/): pesquisa e mapa de implementação (pipeline, sinais, riscos)
- [`docs/adr/`](docs/adr/): decisões de arquitetura e resultados dos portões
- [`docs/superpowers/plans/`](docs/superpowers/plans/): planos 1 e 2, passo a passo com testes
- [`docs/ROADMAP.md`](docs/ROADMAP.md): o que falta, em ordem

## Licença

MIT.
