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

**Núcleo em Rust** (um daemon local), com **modelos pesados como workers atrás de um contrato**, **interface em React + TypeScript dentro de uma casca Electron**, e **Claude Code como forma principal de uso** (MCP + skills).

1. **Rust** para o núcleo: domínio, orquestrador de pipeline, cache, sinais (DSP), candidatos, planejamento de render, API HTTP (axum), MCP (`rmcp`), CLI, armazenamento. Motivo central: em um projeto longo, feito por muitas sessões, o **compilador é o guardião dos contratos** entre módulos; enums modelam os estados do pipeline; e o servidor concorrente não tem GIL.
2. **Workers de ML** (Whisper, e o que não tiver caminho nativo bom) como **processos separados** falando um protocolo estável (JSON por stdin/stdout ou HTTP local), supervisionados pelo núcleo (readiness, timeout, reinício com backoff). Começam em Python (`faster-whisper`), porque é o caminho mais seguro nesta máquina; podem virar nativos (`ort`, `whisper-rs`) depois, sem mexer no resto, porque a porta (`Transcriber`, `FaceDetector`) é a mesma.
3. **Python também como laboratório**, nunca como dependência do núcleo: experimentos de sinais e pesos leem os mesmos arquivos (`.npy`) e o mesmo SQLite. Sinais externos entram por **contrato de dados** (um executável que emite uma série), sem recompilar o núcleo.
4. **Interface:** React + Vite + TypeScript (escolha do Matheus: é o que ele mais conhece e o que melhor funciona com geração por IA). Servida pelo daemon e empacotada em uma **casca Electron** (Matheus não quer abrir linha de comando). REST + SSE para progresso. Só em `127.0.0.1`, com token por sessão.
   - **Electron:** processo principal mínimo (janela, bandeja, instância única, início e término do daemon); `contextIsolation` ligado, sem `nodeIntegration`, preload mínimo; o daemon Rust vai como recurso do instalador (`electron-builder`). A UI não tem lógica de negócio, só fala com a API.
   - **A UI é a superfície de controle completa** (configuração, disparar jobs, entradas, validação, revisão de cortes, gabarito, avaliação): tudo que o MCP e a CLI fazem, a UI faz.
   - **Também roda no navegador comum**, servida pelo daemon em `http://127.0.0.1:<porta>/`, para o Claude operá-la via Claude in Chrome. Por isso **nenhuma função da UI pode depender de API exclusiva do Electron** (o preload só oferece conveniências como bandeja e seletor de pasta nativo, sempre com equivalente na web).
   - **Operável por agente:** HTML semântico, `aria-label` e `data-testid` estáveis em todo controle, estados explícitos (carregando, erro, vazio), sem ações só por arrastar, e o token de sessão aceito por URL (`#token=`) para abrir no navegador sem login.
   - **Pré-visualização:** a UI toca **proxies MP4 H.264** gerados no ingest, nunca o MKV/HEVC original. O Chromium não toca MKV e o suporte a HEVC é incerto (não verificado). O proxy é transcodificado depois da gravação, fora do jogo.
5. **Armazenamento:** SQLite (WAL) para metadados, gabarito e avaliações; séries temporais e proxies em arquivos `.npy` (legíveis direto por NumPy no laboratório). Sem Postgres.
6. **FFmpeg como processo externo** (não em processo): robusto, já tem NVENC/NVDEC, isola falhas. A camada `cm-media` encapsula o comando e valida a saída.

### Por que não as alternativas

- **Tudo em Python:** entrega a v1 mais rápido e o cálculo não é gargalo. Perde em contratos verificáveis ao longo de muitas sessões, em concorrência de servidor e em distribuição. É a opção certa se o projeto fosse um script de pesquisa. Não é para este escopo.
- **Go:** ótimo servidor e iteração rápida, mas sem tipos soma (estados do pipeline ficam frágeis), sem ecossistema de visão e DSP, então todo ML seria worker de qualquer forma e o ganho sobre Rust some. É a segunda opção.
- **Rust 100%, sem workers:** elimina a fronteira de processos, mas coloca o build de CUDA/Whisper (CMake, LLVM, CUDA 13.0 vs. binários do `ort`) no caminho crítico antes de qualquer valor entregue.

## Uso principal: Claude Code

O Matheus quer abrir o Claude Code e pedir "gera os cortes do meu último vídeo" e o sistema se virar. Consequências de projeto:
- **O MCP é cidadão de primeira classe**, não um extra. Operações de alto nível: `latest_recording`, `generate_clips(recording | "latest", options)`, `get_job`, `list_clips`, `get_manifest`. "Último vídeo" = gravação mais recente na pasta do OBS, resolvida pelo sistema, não pelo agente.
- **Jobs assíncronos:** `generate_clips` devolve um `job_id` imediatamente; progresso por `get_job` e por notificações de progresso do MCP. Chamadas longas não ficam presas em um timeout.
- **O daemon é a fonte única de estado.** O processo MCP (stdio) é um cliente fino: se o daemon não estiver rodando, **ele o inicia**, então funciona mesmo com o Electron fechado.
- **Skill `gerar-cortes`:** Markdown que ensina o agente o fluxo (achar o último vídeo, gerar, conferir o manifesto, mostrar o resultado) usando só as tools do MCP.
- **Opcional, coerente com "tudo automático":** vigia da pasta do OBS: quando uma gravação termina (arquivo estável), o ingest começa sozinho.

## Arquitetura (hexagonal)

```
interfaces:  Electron (UI React) | MCP (Claude Code) | HTTP API | CLI | skills (markdown)
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
- **Sem prompts interativos em nenhum caminho:** tudo que o agente ou a UI aciona roda sem pedir nada ao usuário; erros voltam como dados estruturados.
- **Skills e tools:** o MCP expõe as operações; as skills são arquivos Markdown que ensinam um agente a usar CLI/MCP. Ambas são camadas finas sobre a mesma API.

### Estrutura do repositório

```
crates/  cm-domain  cm-store  cm-media  cm-signals  cm-webcam  cm-candidates
         cm-render  cm-ranker  cm-pipeline  cm-api  cm-mcp  cm-cli
ui/      (React + Vite + TS)  shell/ (Electron)
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
6. **MCP + skill `gerar-cortes` (uso principal via Claude Code)**, antes da UI.
7. API + UI React/Electron (revisão de cortes, ferramenta de gabarito).
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

## Portões (resultados)

- **S2 (toolchain Rust), 2026-11-18: PASSOU.** Rust 1.100.0 via rustup/winget, linker MSVC (VS Build Tools 18) presente. Workspace de 4 crates: build limpo 26 s (8,8 s de compilação); incremental ao tocar `cm-domain` 0,26 s (crates ainda vazios: reavaliar quando houver dependências pesadas). Critérios: limpo ≤ 3 min, incremental ≤ 15 s.
- **Ingest em gravação real (Marco 0), 2026-11-18: PASSOU.** `cortes ingest` em `2025-11-30 14-45-52.mkv` (15,25 GB, 1597,6 s, 1080p60 HEVC, 1 faixa): **119,8 s** (~127 MB/s lidos do HDD), contra a meta de ≤ 1,3x a leitura crua (~157 s a 97 MB/s → ≤ 204 s). Saídas: áudio 102,2 MB (= duração × 16 kHz × 4 B, correto) e proxy de 23,8 MB (413 quadros-chave; essa gravação antiga tem quadro-chave a cada ~3,9 s, não 2 s). Usei esse arquivo e não o `2026-07-27 23-05-35.mkv` do plano porque este já estava no cache do Windows.

- **S1 (ort + YuNet em Rust), 2026-11-18: PASSOU em CPU.** `spikes/s1-ort-yunet` (`ort` 2.0.0-rc.15 + `face_detection_yunet_2023mar.onnx`, sha256 `8f2383e4…2fa4`, entrada fixa 640x640 BGR 0–255, decodificação própria dos mapas cls/obj/bbox + NMS). Nos 4 quadros 1080p reais das gameplays (35% da duração): rosto único detectado em 4/4, confiança 0,90–0,92, caixas conferidas visualmente sobre a webcam. Latência: **CPU 5,6–6,4 ms/quadro** (inclui cópia da entrada); DirectML 3–20 ms, instável e sem ganho real. Decisão: **detector de rosto em Rust/CPU, sem worker Python**; DirectML/CUDA ficam fora do caminho crítico. Limitações: modelo exige 640x640 (letterbox com escala), então rostos pequenos em 1080p perdem resolução — a detecção da webcam deve rodar em recorte da região candidata (plano 2). CUDA não testado (exige cuDNN).

- **Plano 2 (webcam + render 9:16), 2026-11-19: PASSOU nas 4 gameplays.** `cortes webcam` e `cortes render-sample` sobre as gravações reais (16 quadros de 1080p por gravação, detector em CPU; ~25 s por gravação em build debug, dominado pela leitura do HDD). Novembro (webcam com moldura, esquerda/meio): retângulo `0,438 404x280`, IoU 0,976 nas duas, confiança 0,86 e 0,91. Julho (webcam recortada da pessoa, sem moldura, canto inferior esquerdo): retângulo anatômico a partir do rosto, IoU 0,955 e 0,909 contra a caixa justa da pessoa, contenção ≥ 0,974. O spec supunha um retângulo em todas as sessões; em julho não existe, então o critério ficou 0,9 (novembro) e 0,75 + contenção ≥ 0,95 (julho). O gabarito de julho foi lido a olho e o retângulo anatômico foi calibrado nos mesmos 2 vídeos: o Matheus revisa na página de avaliação. Descobertas na validação: (1) o limiar de rosto 0,6 deixa passar falsos positivos de jogo (um de 0,74), então o rosto-semente é o que mais se repete na mesma posição entre os quadros; (2) uma borda interna a 6 px do quadro (suporte de microfone) enganava o lado esquerdo, resolvido encostando na borda do quadro quando a distância é ≤ 2% da largura; (3) lados internos entram 2 px para não vazar jogo na faixa da webcam; (4) `-cq 19` do NVENC gerava 42 Mbps (52 MB por 10 s), ficou `-cq 21 -maxrate 14M` (~18 MB por 10 s). Render 9:16: 1080x1920, webcam em cima (752 a 842 px), jogo embaixo sem tocar a webcam, NVENC com fallback para x264.

- **Ajuste após revisão do Matheus (19/11/2026):** nas gravações de julho o gabarito foi marcado como "quase" e a faixa da webcam como "muito colada em cima na cabeça". Subi a folga acima da cabeça de 0,33 para 0,6 da altura do rosto e o gabarito de julho em ~45 px (`735` e `745`). Gate: julho IoU 0,962 e 0,911, contenção 0,995 e 0,975; novembro inalterado (0,976). Notas do Matheus: novembro 5/5/5, julho retângulo 4, layout 5, qualidade 5, geral 5, "serve para postar" 5; próximo foco escolhido: seleção de momentos por sinais de áudio.
