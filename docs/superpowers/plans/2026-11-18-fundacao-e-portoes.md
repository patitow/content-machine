# Fundação e portões de decisão: plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ter um esqueleto Rust que anda de ponta a ponta (`cortes latest` e `cortes ingest` sobre uma gravação real do OBS, em uma só passagem de leitura, com cache) e fechar os portões S1 (ort + YuNet) e S2 (toolchain Rust) do ADR 0001.

**Architecture:** Workspace Cargo com `cm-domain` (tipos puros), `cm-media` (ffprobe/ffmpeg atrás de um trait), `cm-store` (SQLite) e `cm-cli`. FFmpeg roda como processo externo; o ingest produz áudio 16 kHz mono por faixa e proxy de vídeo 320x180 em gray na mesma invocação, gravando em diretório temporário e movendo de forma atômica para o cache.

**Tech Stack:** Rust estável (fixado em `rust-toolchain.toml`), `rusqlite` (bundled, WAL), `serde`/`serde_json`, `blake3`, `clap`, `thiserror`, `tempfile`, `toml`; FFmpeg/FFprobe do PATH (chocolatey); `ort` apenas no spike S1.

**Spec:** `docs/adr/0001-arquitetura-e-linguagem.md` (arquitetura, portões, marcos) e `docs/superpowers/specs/2026-11-18-cortes-locais-design.md` (requisitos).

## Escopo deste plano

Este plano cobre **só o Marco 0 e os portões S1/S2**. O resto do projeto vira planos próprios, nesta ordem: (2) webcam + render, (3) sinais + candidatos + avaliação, (4) worker Whisper + fronteiras, (5) daemon + API + MCP + skill `gerar-cortes`, (6) UI React + Electron (superfície de controle completa, também no navegador).

## Global Constraints

- Rust **estável**, versão fixada em `rust-toolchain.toml`; `cargo fmt --check`, `cargo clippy -- -D warnings` e `cargo test` devem passar a cada commit.
- Identificadores de código em inglês; textos para o usuário (CLI, erros mostrados) em português.
- Cache de proxies em `E:\.VIDEOS GRAVADOS\content-machine-cache` por padrão (o C: está 90% cheio); configurável.
- Pasta do OBS: `E:\.VIDEOS GRAVADOS\OBS` (começa com ponto, tem espaço e acento: precisa funcionar).
- Faixas de áudio do OBS: 4 faixas = `Mix`, `Jogo (padrao)`, `Comunicacao`, `Mic`. Gravações antigas têm 1 faixa titulada `Track1` (= Mix). Há vídeos **sem áudio** (`videoplayback.mp4`).
- Ingest lê o arquivo **uma única vez** do HDD (~97 MB/s medidos); meta de tempo: ≤ 1,3x o tempo de leitura crua (12,2 GB ≈ 126 s, então ≤ 165 s).
- SQLite em modo WAL, migrações por `PRAGMA user_version`.
- Nenhum caminho pede entrada interativa; erros saem como `Result` tipado.
- Commits sem `Co-Authored-By` e sem selo de IA (regra global do Matheus). Fluxo de git: branch de feature a partir de `main`.

## Review Focus

1. **Gravação ainda sendo escrita ou com 0 byte** (a pasta do OBS tem vários `.mkv` de 0 GB): `latest` não pode escolhê-la. Teste na Task 5.
2. **Mesmo nome em `.mkv` e `.mp4`** (ex.: `2026-06-17 21-11-27`): `latest` devolve um só, preferindo o `.mkv`. Teste na Task 5.
3. **Vídeo sem faixa de áudio:** `probe` e `ingest` funcionam, sem arquivos de áudio. Teste nas Tasks 2 e 4.
4. **Caminho com ponto inicial, espaços e acentos:** `probe`, hash e ingest funcionam. Teste na Task 2.
5. **Ingest interrompido ou disco cheio:** nunca deixa saída parcial que o cache trate como válida. Teste na Task 4.

---

### Task 1: Toolchain, workspace e portão S2

**Files:**
- Create: `Cargo.toml` (workspace), `rust-toolchain.toml`, `.gitignore`, `CLAUDE.md`, `scripts/check.ps1`
- Create: `crates/cm-domain/{Cargo.toml,src/lib.rs}`, `crates/cm-media/{Cargo.toml,src/lib.rs}`, `crates/cm-store/{Cargo.toml,src/lib.rs}`, `crates/cm-cli/{Cargo.toml,src/main.rs}`
- Modify: `docs/adr/0001-arquitetura-e-linguagem.md` (registrar resultado do S2)

**Interfaces:**
- Produces: workspace compilável; `scripts/check.ps1` roda fmt, clippy e test e sai com código ≠ 0 se algo falhar.

- [ ] **Step 1: Instalar Rust.** `winget install Rustlang.Rustup` (ou instalador oficial), depois `rustup default stable`. Verificar: `cargo --version` e `rustc --version` imprimem versões; `rustup show` indica alvo `x86_64-pc-windows-msvc`. Se faltar o linker MSVC, instalar "Visual Studio Build Tools" com o componente C++ e repetir.
- [ ] **Step 2: Criar o workspace** com os 4 crates vazios; `rust-toolchain.toml` fixando a versão estável instalada e `components = ["rustfmt","clippy"]`; `.gitignore` com `target/`, `*.db`, `*.db-wal`, `*.db-shm`.
- [ ] **Step 3: `scripts/check.ps1`** executa `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, parando no primeiro erro.
- [ ] **Step 4: `CLAUDE.md`** com: comandos (`scripts/check.ps1`), a regra "contratos antes de implementação", as Global Constraints deste plano, e a ordem dos planos.
- [ ] **Step 5: Portão S2.** Medir `cargo build --workspace` limpo e uma recompilação incremental depois de tocar `cm-domain/src/lib.rs`. Critério: build limpo ≤ 3 min, incremental ≤ 15 s. Registrar os tempos e o veredito (passou/falhou) na seção "Portões" do ADR. **Se falhar, parar e reportar ao Matheus** (reavaliar Go, conforme ADR).
- [ ] **Step 6: Verificar** `scripts/check.ps1` → sai com 0.
- [ ] **Step 7: Commit** `chore: workspace Rust, toolchain fixa e script de verificação`.

### Task 2: Tipos de domínio e `probe`

**Files:**
- Modify: `crates/cm-domain/src/lib.rs`, `crates/cm-media/src/lib.rs`
- Create: `crates/cm-domain/src/recording.rs`, `crates/cm-media/src/probe.rs`, `crates/cm-media/src/runner.rs`
- Test: `crates/cm-domain/src/recording.rs` (módulo `tests`), `crates/cm-media/tests/probe.rs`

**Interfaces:**
- Produces (`cm-domain`): `enum TrackRole { Mix, Game, Comms, Mic, Unknown }`; `struct AudioTrack { index: u32, title: Option<String>, role: TrackRole }`; `struct Recording { path: PathBuf, duration_s: f64, width: u32, height: u32, fps: f64, audio_tracks: Vec<AudioTrack> }`; `fn assign_roles(titles: &[Option<String>]) -> Vec<TrackRole>`.
- Produces (`cm-media`): `trait CommandRunner { fn run(&self, program: &str, args: &[OsString]) -> Result<CommandOutput, MediaError>; }` com `SystemRunner`; `fn probe(runner: &dyn CommandRunner, path: &Path) -> Result<Recording, MediaError>`; `enum MediaError` (`thiserror`) com variantes `Spawn`, `NonZeroExit { code, stderr }`, `Parse(String)`, `NoVideoStream`.

- [ ] **Step 1: Testes de `assign_roles`** (código): `["Mix","Jogo (padrao)","Comunicacao","Mic"]` → `[Mix, Game, Comms, Mic]`; `[Some("Track1")]` → `[Mix]`; quatro títulos `TrackN` → `[Mix, Game, Comms, Mic]` (posicional); `[]` → `[]`; dois títulos desconhecidos → `[Unknown, Unknown]`.
- [ ] **Step 2: Rodar** `cargo test -p cm-domain` → falha (função inexistente).
- [ ] **Step 3: Implementar** `assign_roles` e os tipos. Regra: títulos conhecidos mandam (comparação sem acento e sem caixa); senão, 1 faixa = Mix e 4 faixas = ordem Mix, Game, Comms, Mic; qualquer outra contagem = Unknown.
- [ ] **Step 4: Testes de `probe`** em `tests/probe.rs`, gerando fixtures com `ffmpeg -f lavfi` em um `tempdir`: (a) MKV com vídeo + 4 trilhas de áudio tituladas → `audio_tracks.len()==4` e papéis certos; (b) MP4 **sem áudio** → `audio_tracks` vazio; (c) arquivo dentro de pasta `.pasta com espaço e acentuação ção` → `probe` funciona; (d) arquivo que não é mídia → `Err(MediaError::Parse(_))` ou `NonZeroExit`; (e) só áudio, sem vídeo → `Err(MediaError::NoVideoStream)`.
- [ ] **Step 5: Rodar** os testes → falham.
- [ ] **Step 6: Implementar `probe`** chamando `ffprobe -v error -print_format json -show_format -show_streams`, desserializando com `serde`; `fps` vem de `r_frame_rate` (fração).
- [ ] **Step 7: Rodar** `scripts/check.ps1` → passa.
- [ ] **Step 8: Commit** `feat: tipos de gravação, papéis de faixa e probe via ffprobe`.

### Task 3: Fingerprint de conteúdo e armazenamento SQLite

**Files:**
- Create: `crates/cm-media/src/fingerprint.rs`, `crates/cm-store/src/lib.rs`, `crates/cm-store/src/migrations.rs`
- Test: `crates/cm-media/tests/fingerprint.rs`, `crates/cm-store/tests/store.rs`

**Interfaces:**
- Consumes: `Recording`, `TrackRole` (Task 2).
- Produces: `fn fingerprint(path: &Path) -> Result<String, MediaError>` (hex blake3 de: tamanho + 1 MiB do início + 1 MiB do meio + 1 MiB do fim; **não** lê o arquivo inteiro, por causa do HDD); `struct Store`; `Store::open(path: &Path) -> Result<Store, StoreError>`; `Store::upsert_recording(&self, fingerprint: &str, rec: &Recording) -> Result<(), StoreError>`; `Store::find_recording(&self, fingerprint: &str) -> Result<Option<Recording>, StoreError>`.

- [ ] **Step 1: Testes de `fingerprint`:** mesmo conteúdo em dois caminhos → mesmo hash; alterar 1 byte no fim → hash muda; alterar 1 byte no meio → muda; arquivo menor que 3 MiB funciona; arquivo inexistente → `Err`.
- [ ] **Step 2: Rodar** → falha. **Step 3: Implementar** `fingerprint` com `std::io::Seek`.
- [ ] **Step 4: Testes de `Store`:** `open` num banco novo cria o esquema e `PRAGMA journal_mode` retorna `wal`; `upsert` seguido de `find` devolve um `Recording` igual (incluindo papéis das faixas); `upsert` duas vezes não duplica; `find` de hash desconhecido → `None`; reabrir o banco preserva os dados; banco com `user_version` maior que o suportado → `Err(StoreError::SchemaTooNew)`.
- [ ] **Step 5: Rodar** → falha. **Step 6: Implementar** migrações por `PRAGMA user_version` (versão 1: tabelas `recording` e `audio_track`).
- [ ] **Step 7: Rodar** `scripts/check.ps1` → passa. **Step 8: Commit** `feat: fingerprint de conteúdo e armazenamento SQLite com migrações`.

### Task 4: Ingest em uma passagem com cache atômico

**Files:**
- Create: `crates/cm-media/src/ingest.rs`
- Test: `crates/cm-media/tests/ingest.rs`

**Interfaces:**
- Consumes: `CommandRunner`, `probe`, `fingerprint`, `Recording`.
- Produces: `struct IngestOutput { fingerprint: String, audio_files: Vec<(u32, PathBuf)>, video_proxy: PathBuf, from_cache: bool }`; `const INGEST_VERSION: u32`; `fn ingest(runner: &dyn CommandRunner, rec: &Recording, fingerprint: &str, cache_dir: &Path) -> Result<IngestOutput, MediaError>`.

- [ ] **Step 1: Testes** (com um `FakeRunner` que registra as chamadas, e um teste de integração com ffmpeg real e fixture `lavfi` de ~10 s com 4 faixas): (a) uma única invocação do `ffmpeg` produz todos os arquivos (`assert_eq!(runner.ffmpeg_calls(), 1)`); (b) chamar `ingest` duas vezes → segunda devolve `from_cache == true` e **zero** novas chamadas; (c) vídeo sem áudio → `audio_files` vazio e proxy existe; (d) `ffmpeg` falha no meio → o diretório de cache **não** contém saída com a chave final, e uma nova chamada tenta de novo (não confunde saída parcial com cache); (e) mudar `INGEST_VERSION` invalida o cache; (f) integração: áudio de cada faixa tem ≈ `duração × 16000 × 4` bytes (f32le mono), e o proxy de vídeo tem `N` quadros de 320x180 gray com `N ≈ duração / keyint` (fixture com keyint de 2 s).
- [ ] **Step 2: Rodar** → falha.
- [ ] **Step 3: Implementar `ingest`.** Uma invocação: `-skip_frame nokey` antes do `-i` (só quadros-chave no vídeo), um `-map 0:a:N -ac 1 -ar 16000 -f f32le` por faixa, e `-map 0:v:0 -vf scale=320:180,format=gray -fps_mode passthrough -f rawvideo`. Escrever em `<cache>/<fingerprint>.<INGEST_VERSION>.tmp/` e `rename` para o nome final só depois de sucesso; um arquivo `manifest.json` dentro marca a saída como completa.
- [ ] **Step 4: Rodar** `scripts/check.ps1` → passa. **Step 5: Commit** `feat: ingest em uma passagem com cache atômico`.

### Task 5: CLI, configuração e `latest`

**Files:**
- Create: `crates/cm-cli/src/{main.rs,config.rs,latest.rs}`
- Test: `crates/cm-cli/tests/latest.rs`, `crates/cm-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `probe`, `fingerprint`, `Store`, `ingest`.
- Produces: `struct Config { obs_dir: PathBuf, cache_dir: PathBuf, db_path: PathBuf }` carregada de `%APPDATA%\content-machine\config.toml` (padrões: `E:\.VIDEOS GRAVADOS\OBS`, `E:\.VIDEOS GRAVADOS\content-machine-cache`, `<cache>\cortes.db`); `fn latest_recording(dir: &Path, now: SystemTime, settle: Duration) -> Result<Option<PathBuf>, io::Error>`; subcomandos `cortes latest` e `cortes ingest <caminho|latest>`.

- [ ] **Step 1: Testes de `latest_recording`** em `tests/latest.rs` (arquivos criados em `tempdir` com `filetime` para controlar `mtime`): (a) devolve o de `mtime` mais recente entre `.mkv`/`.mp4`; (b) **ignora arquivo de 0 byte**; (c) **ignora arquivo com `mtime` dentro de `settle`** (ainda sendo gravado) e devolve o anterior; (d) `.mkv` e `.mp4` com o mesmo nome base → devolve o `.mkv`; (e) ignora subpastas (ex.: `SourceRecord`), `.png`, `.bak`; (f) pasta vazia → `None`; (g) pasta inexistente → `Err`.
- [ ] **Step 2: Rodar** → falha. **Step 3: Implementar** `latest_recording` e `Config::load` (arquivo ausente → padrões).
- [ ] **Step 4: Testes de CLI** (`assert_cmd`): `cortes latest` imprime o caminho em pasta de teste; `cortes ingest <fixture>` imprime o resumo em português (duração, resolução, faixas e papéis) e grava no SQLite; segunda execução imprime "já em cache"; `cortes ingest inexistente` sai com código ≠ 0 e mensagem em português, sem pânico.
- [ ] **Step 5: Rodar** → falha. **Step 6: Implementar** com `clap` (derive) ligando as peças.
- [ ] **Step 7: Verificação em dado real.** Rodar `cortes ingest "E:\.VIDEOS GRAVADOS\OBS\2026-07-27 23-05-35.mkv"` e anotar o tempo. Critério: ≤ 165 s (≤ 1,3x a leitura crua, desde que o arquivo não esteja no cache do Windows) e saídas coerentes (áudio ≈ 64 MB, proxy com ≈ duração/2 quadros). Registrar o tempo medido e o veredito na seção "Portões" do ADR. Se estourar, reportar com os números em vez de ajustar o critério.
- [ ] **Step 8: Rodar** `scripts/check.ps1` → passa. **Step 9: Commit** `feat: CLI com latest e ingest, configuração e verificação em gravação real`.

### Task 6: Portão S1 (ort + YuNet)

**Files:**
- Create: `spikes/s1-ort-yunet/{Cargo.toml,src/main.rs,README.md}` (crate **fora** do workspace principal)
- Modify: `docs/adr/0001-arquitetura-e-linguagem.md` (resultado do S1)

**Interfaces:**
- Produces: veredito S1 (passou/falhou) com tempos por quadro, registrado no ADR. Nenhum código de produção depende deste crate.

- [ ] **Step 1: Extrair quadros** de teste: 1 quadro (aos 35% da duração) de cada uma das 4 gameplays (`2025-11-30 13-54-24`, `2025-11-30 14-45-52`, `2026-07-27 22-26-47`, `2026-07-27 23-05-35`) em PNG 1920x1080, com `ffmpeg -ss <t> -i <arquivo> -frames:v 1`.
- [ ] **Step 2: Baixar o modelo** YuNet (`face_detection_yunet_*.onnx` do OpenCV Zoo) para uma pasta fora do repositório; registrar a URL e o hash SHA-256 no README do spike.
- [ ] **Step 3: Implementar o spike**: carrega o ONNX com `ort`, roda em cada quadro, imprime caixas (x, y, w, h, confiança) e o tempo por inferência. Tentar primeiro CUDA; se falhar por incompatibilidade de DLL/versão (CUDA 13.0 instalado), tentar DirectML, depois CPU.
- [ ] **Step 4: Critério de passe:** o rosto é detectado (confiança ≥ 0,6) dentro da região onde a webcam aparece (conferir visualmente em cada um dos 4 quadros e anotar as caixas esperadas no README), com ≤ 50 ms por quadro no provedor escolhido.
- [ ] **Step 5: Registrar no ADR** o provedor que funcionou, os tempos e o veredito. **Se falhar,** registrar que o detector de rosto vira worker Python (a decisão do núcleo em Rust se mantém, conforme o ADR) e reportar ao Matheus.
- [ ] **Step 6: Commit** `spike: S1 ort + YuNet e resultado dos portões no ADR`.

---

## Auto-revisão

- **Cobertura do spec:** ADR marco 0 (esqueleto, ffprobe, SQLite, ingest em uma passagem com cache) → Tasks 1 a 5; portões S1 e S2 → Tasks 6 e 1. Requisitos de "último vídeo" e de faixas (design/ADR) → Tasks 2 e 5. Webcam, render, sinais, MCP e UI ficam nos planos seguintes (listados no Escopo).
- **Consistência de tipos:** `Recording`, `TrackRole`, `AudioTrack`, `CommandRunner`, `MediaError`, `Store`, `IngestOutput`, `Config` têm uma única definição e são consumidos pelos nomes das Interfaces.
- **Review Focus:** os 5 itens têm teste nas Tasks 2, 4 e 5.
- **Proporção:** o plano descreve decisões, assinaturas e asserções; os corpos ficam com quem implementa.
