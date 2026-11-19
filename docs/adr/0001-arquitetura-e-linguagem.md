# ADR 0001: arquitetura e linguagem

Data: 2026-11-18. Status: **proposta fundamentada, aguardando o ok do Matheus**. Substitui a recomendação "Python na v1" de `docs/research/2026-11-18-mapa-de-implementacao.md`.

## Contexto

O escopo cresceu: não é só um script de cortes. Precisa de pipeline de análise, avaliação contra gabarito, CLI, **API + interface web própria**, **servidor MCP**, **skills e tools** para agentes, adaptadores de IA trocáveis, e vai ser construído **aos poucos, por muitas sessões**, por pessoas e por IA, sem que ninguém segure o código inteiro na cabeça. Qualidade ao longo do tempo é o critério principal.

## O que medi nesta máquina (gravação real de 12,2 GB, 1001 s, 1080p60 HEVC)

| Medida | Resultado | Leitura |
|---|---|---|
| Ler o arquivo e extrair áudio 16 kHz | 126 s (~97 MB/s) | **Limitado pelo disco**: o E: é um HDD Seagate de 2 TB. Linguagem é irrelevante. |
| Decodificar vídeo e gerar proxy 320x180 a 4 fps (120 s de vídeo) | 27 s com NVDEC, 31 s por CPU | Limitado pela **decodificação do FFmpeg** (~4x tempo real). Linguagem é irrelevante. |
| Sinais de áudio em NumPy (RMS + FFT + fluxo espectral, 1002 s) | **0,37 s** | O código Python é desprezível (~2700x tempo real). |

Conclusão: a objeção "Python não tem performance" **não se confirma para o cálculo** deste pipeline: o tempo está no disco, no decode e na GPU. Mas isso **não decide a questão**, porque a escolha também depende de manutenção, concorrência de servidor, tipagem e evolução. Dois achados de arquitetura saem daqui:
- **Ingest em uma só passagem:** ler o arquivo do HDD uma vez e extrair áudio e proxies no mesmo comando FFmpeg, guardando o resultado em cache. Ler várias vezes custa minutos por sessão.
- Para os proxies de vídeo, avaliar `-skip_frame nokey` (só quadros-chave, ~0,5 fps com keyint de 2 s) para acelerar o decode ordens de grandeza. O disco C: está 90% cheio: o cache vai no E:.

## Critérios (peso para este projeto)

| Critério | Peso | Python | Rust | Go |
|---|---|---|---|---|
| Contratos entre módulos verificados pelo compilador (muitas sessões, sem contexto global) | alto | médio (pyright estrito + pydantic) | **alto** | médio-alto |
| Modelar estados do pipeline sem ambiguidade (enums com dados, `Result`) | alto | médio | **alto** | baixo (sem tipos soma) |
| Concorrência de servidor (jobs, SSE, UI, MCP ao mesmo tempo) sem GIL | alto | médio | **alto** | **alto** |
| Ecossistema de ML/visão (Whisper, rosto, OCR) | alto | **alto** | médio (`ort`, `whisper-rs`, `ct2rs`) | baixo |
| Velocidade de iteração em heurísticas e avaliação | alto | **alto** | médio | médio |
| Binário único, instalação simples | médio | baixo | **alto** | **alto** |
| DSP em processo (sinais de horas de áudio) | médio | alto (NumPy) | **alto** | médio |
| Tempo de compilação / ciclo de desenvolvimento | médio | **alto** | baixo | **alto** |
| Risco de build no Windows desta máquina | médio | **baixo** | médio-alto (sem `cargo`, `cmake`, LLVM; CUDA 13.0) | baixo |

Evidências externas: SDKs oficiais de MCP de nível 1 existem para Python, Go e Rust ([tiers](https://modelcontextprotocol.info/community/sdk-tiers/)); `whisper-rs` exige CUDA Toolkit, Visual Studio C++/Clang, CMake e LLVM no Windows ([BUILDING.md](https://github.com/tazz4843/whisper-rs/blob/master/BUILDING.md)); `ort` publica binários para CUDA 13 e há conflitos de DLL conhecidos no Windows ([docs](https://ort.pyke.io/perf/execution-providers)); `ct2rs` (bindings do CTranslate2) não tem confirmação de CUDA no Windows nas fontes que achei; o padrão "núcleo em Rust, Python só onde o ecossistema exige, atrás de um contrato estável e supervisionado" aparece em projetos reais ([resumo](https://daily.dev/posts/the-rust-sidecar-pattern-that-fixes-python-ai-s-biggest-weakness-wqtkvgucc)), com o custo de documentar e testar as duas pontas.

## Decisão

**Núcleo em Rust**, com **modelos pesados como workers atrás de um contrato**, e **interface web em TypeScript**.

1. **Rust** para o núcleo: domínio, orquestrador de pipeline, cache, sinais (DSP), candidatos, planejamento de render, API HTTP (axum), MCP (`rmcp`), CLI, armazenamento. Motivo central: em um projeto longo, feito por muitas sessões, o **compilador é o guardião dos contratos** entre módulos; enums modelam os estados do pipeline; e o servidor concorrente não tem GIL.
2. **Workers de ML** (Whisper, e o que não tiver caminho nativo bom) como **processos separados** falando um protocolo estável (JSON por stdin/stdout ou HTTP local), supervisionados pelo núcleo (readiness, timeout, reinício com backoff). Começam em Python (`faster-whisper`), porque é o caminho mais seguro nesta máquina; podem virar nativos (`ort`, `whisper-rs`) depois, sem mexer no resto, porque a porta (`Transcriber`, `FaceDetector`) é a mesma.
3. **Python também como laboratório**, nunca como dependência do núcleo: experimentos de sinais e pesos leem os mesmos arquivos (`.npy`) e o mesmo SQLite. Sinais externos entram por **contrato de dados** (um executável que emite uma série), sem recompilar o núcleo.
4. **Interface web:** SvelteKit (ou React) + Vite, servida pelo núcleo e embutida no binário (`rust-embed`) em release; em desenvolvimento, proxy para o Vite. REST + SSE para progresso. Vídeo por HTTP com `Range`. Só em `127.0.0.1`, com token. Tauri fica como opção futura de empacotamento, não de partida.
5. **Armazenamento:** SQLite (WAL) para metadados, gabarito e avaliações; séries temporais e proxies em arquivos `.npy` (legíveis direto por NumPy no laboratório). Sem Postgres.
6. **FFmpeg como processo externo** (não em processo): robusto, já tem NVENC/NVDEC, isola falhas. A camada `cm-media` encapsula o comando e valida a saída.

### Por que não as alternativas

- **Tudo em Python:** entrega a v1 mais rápido e o cálculo não é gargalo. Perde em contratos verificáveis ao longo de muitas sessões, em concorrência de servidor e em distribuição. É a opção certa se o projeto fosse um script de pesquisa. Não é para este escopo.
- **Go:** ótimo servidor e iteração rápida, mas sem tipos soma (estados do pipeline ficam frágeis), sem ecossistema de visão e DSP, então todo ML seria worker de qualquer forma e o ganho sobre Rust some. É a segunda opção.
- **Rust 100%, sem workers:** elimina a fronteira de processos, mas coloca o build de CUDA/Whisper (CMake, LLVM, CUDA 13.0 vs. binários do `ort`) no caminho crítico antes de qualquer valor entregue.

## Arquitetura (hexagonal)

```
interfaces:  CLI | HTTP API + Web UI | MCP | skills (markdown que chamam CLI/MCP)
                         │
aplicação:   orquestrador de jobs (DAG) · cache por conteúdo · agendador de recursos (GPU, disco)
                         │
domínio:     Sessão · Sinal · Candidato · Corte · Retângulo · Pontuação · Gabarito (tipos puros, sem I/O)
                         │
portas:      Media · Transcriber · FaceDetector · Ranker · Store · Clock
adaptadores: ffmpeg · worker Whisper · ort/YuNet · claude/openai-compat/ollama · SQLite+npy
```

- **Cache por conteúdo:** cada etapa tem chave = hash(entradas + parâmetros + versão do código). Mudar pesos recalcula só a fusão, não a transcrição. É o que torna a calibração viável.
- **Agendador de recursos:** semáforos para NVENC (1 render por vez), GPU de Whisper e disco do HDD (1 leitura sequencial por vez).
- **Eventos de progresso** (SSE) emitidos por todas as etapas; a UI e o MCP consomem o mesmo fluxo.
- **Skills e tools:** o MCP expõe as operações; as skills são arquivos Markdown que ensinam um agente a usar CLI/MCP. Ambas são camadas finas sobre a mesma API.

### Estrutura do repositório

```
crates/  cm-domain  cm-store  cm-media  cm-signals  cm-webcam  cm-candidates
         cm-render  cm-ranker  cm-pipeline  cm-api  cm-mcp  cm-cli
ui/      (SvelteKit)
workers/ whisper/ (Python, contrato JSON)   lab/ (notebooks e experimentos)
eval/    gabarito, métricas, teste cego
docs/    adr/  specs/  research/
```

## Como garantir qualidade ao longo do tempo

- **ADRs** para toda decisão estrutural; este é o 0001.
- **Fatias verticais** sempre executáveis de ponta a ponta (esqueleto que anda primeiro), nunca camadas soltas.
- **Definição de pronto:** testes, `clippy`/`rustfmt`, e `cortes eval` sem regressão contra o gabarito.
- **`CLAUDE.md` no repositório** com convenções, comandos e a regra "contratos antes de implementação".
- Testes de propriedade (fusão de sinais, fronteiras) e testes dourados (webcam IoU, comandos FFmpeg).
- Telemetria local estruturada (`tracing`) com duração e resultado por etapa.

## Marcos

0. Esqueleto: workspace, CI local, `ffprobe`, SQLite, ingest em uma passagem com cache.
1. **Webcam** (requisito obrigatório): mapa de estaticidade + bordas, sem ML primeiro; rosto via `ort` depois. Aceite IoU ≥ 0,9.
2. Render 9:16 e 16:9 com retângulo da etapa 1 (encode NVENC, legendas, loudnorm).
3. Sinais + candidatos + `eval` com gabarito.
4. Worker Whisper + fronteiras de frase + legendas.
5. Adaptadores de IA + ranker.
6. API + UI (revisão de cortes, ferramenta de gabarito).
7. MCP + skills.
8. Endurecimento, kill feed do BF6.

## Portões de decisão (o que me faria mudar de ideia)

Antes de fixar o marco 1, duas provas curtas (spikes) com resultado objetivo:
- **S1:** `cargo` + `ort` rodando YuNet em GPU (ou CPU aceitável) nesta máquina em um dia. Se falhar, o detector de rosto vira worker Python e o resto da decisão se mantém.
- **S2:** instalar o toolchain Rust no Windows (MSVC já parece presente via Visual Studio) e compilar um esqueleto em tempo razoável. Se o ciclo de compilação ficar inviável, reavaliar Go.

Se **ambos** falharem, o plano volta para Python com tipagem estrita e a mesma arquitetura hexagonal (a estrutura da decisão não depende da linguagem).

## Consequências

- Mais trabalho inicial (toolchain, contratos, workers) em troca de um núcleo que resiste a muitas sessões de evolução.
- Duas linguagens para manter (Rust + Python nos workers e laboratório): mitigado por contrato estável, supervisão e testes dos dois lados.
- O gabarito do Matheus continua sendo o gargalo de validação, independente da linguagem.
