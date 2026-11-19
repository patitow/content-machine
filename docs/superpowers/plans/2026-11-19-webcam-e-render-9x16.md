# Webcam inteligente e render 9:16: plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `cortes webcam <gravação>` acha o retângulo da webcam na gravação principal (IoU ≥ 0,9 nas 4 gameplays) e `cortes render-sample` gera um trecho 9:16 com a webcam em cima e o jogo embaixo, sem nunca sair com recorte errado.

**Architecture:** `cm-domain` ganha `Rect` e os tipos de detecção. `cm-media` ganha amostragem de quadros em resolução cheia e o montador de comando de render (ffmpeg, NVENC). Um crate novo `cm-vision` guarda o detector de rosto (YuNet via `ort`, promovido do spike S1) e o detector de webcam (rosto como semente, bordas persistentes ao longo do tempo para fechar o retângulo). `cm-store` guarda o retângulo por gravação; `cm-cli` expõe os dois comandos.

**Tech Stack:** o do plano 1 mais `ort` 2.0.0-rc.15 e `image` (só em `cm-vision`); modelo YuNet em `models/`; FFmpeg/NVENC.

**Spec:** `docs/superpowers/specs/2026-11-18-cortes-locais-design.md` (seção "Requisito central: webcam inteligente no 9:16") e `docs/adr/0001-arquitetura-e-linguagem.md` (resultado do portão S1).

## Global Constraints

- Tudo do plano 1 continua valendo: `scripts/check.ps1` verde a cada commit; código em inglês, textos para o usuário em português; sem `Co-Authored-By` nem selo de IA; nenhum caminho interativo.
- **Cadeia de decisão da webcam, nesta ordem:** (1) substituição manual por gravação no `config.toml`; (2) detecção na gravação principal com confiança ≥ 0,6; (3) retângulo salvo de gravação parecida (mesma resolução) se um rosto cair dentro dele em ≥ 60% dos quadros amostrados; (4) sem webcam confiável: corte só do jogo, com aviso. **Nunca** um recorte errado no lugar do passo 4.
- Critério de aceite: nas 4 gameplays, IoU contra o gabarito **≥ 0,9 nas sessões de novembro** (webcam retangular com moldura, `kind: bordered`) e **≥ 0,75 nas de julho** (webcam recortada da pessoa, sem moldura, `kind: cutout`, gabarito = caixa justa da pessoa na união dos quadros; não existe retângulo objetivo, e a caixa nunca pode cortar a pessoa ao meio: contenção ≥ 0,95 da caixa da pessoa). Gravações `2025-11-30 13-54-24`, `2025-11-30 14-45-52`, `2026-07-27 22-26-47`, `2026-07-27 23-05-35`.
- Detecção de rosto: `models/face_detection_yunet_2023mar.onnx` (sha256 `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`), entrada 640x640 BGR 0–255, CPU. Limiar de rosto 0,6 (as detecções falsas medidas ficam ≤ 0,45; as verdadeiras ≥ 0,89).
- Saída 9:16: 1080x1920, `h264_nvenc` p5 `-cq 19`, áudio AAC 192 kbps da faixa de índice 0 (Mix), `-movflags +faststart`. Webcam em cima, largura 1080, altura ≤ 45% (864 px); jogo no resto.
- O recorte do jogo vem do centro do quadro e **nunca intersecta** o retângulo da webcam.
- Funciona em qualquer resolução (1280x720, 1920x1080, 2560x1440): coordenadas do `Rect` em pixels da fonte, cálculos por razão.
- Frames de teste reais ficam em `crates/cm-vision/tests/fixtures/` como JPEG ≤ 150 KB (repositório privado; contém o rosto do Matheus).

## Review Focus

1. **Webcam recortada da pessoa, sem moldura, encostada na borda do quadro** (as sessões de julho: canto inferior esquerdo, fundo removido, o jogo aparece em volta): não há borda persistente para achar; o detector cai num retângulo anatômico a partir do rosto (largura ≈ 3 larguras de rosto, do topo da cabeça até a borda inferior) e marca `kind` cutout. Teste na Task 4 com sintético e no gate real.
2. **Pessoa sai de cena ou o rosto some em vários quadros:** a detecção usa só os quadros com rosto; com menos de 5 quadros com rosto devolve `NotFound`, nunca um retângulo chutado. Teste na Task 4.
3. **Vídeo falado em tela cheia (reuniões, aulas):** o "rosto" ocupa o quadro; retângulo > 25% da área vira `NotFound` (sem overlay), e o render cai no corte só do jogo/central com aviso. Teste na Task 4.
4. **Webcam perto do centro:** o recorte do jogo desloca ou estreita para nunca intersectar o retângulo. Teste na Task 6.
5. **Resolução diferente de 1080p** e **webcam de proporção incomum** (quadrada, 4:3, 21:9): o render mantém a faixa da webcam ≤ 45% da altura cortando o excesso vertical, centrado no rosto. Teste na Task 6.

---

### Task 1: `Rect`, tipos de detecção e gabarito das 4 gameplays

**Files:**
- Create: `crates/cm-domain/src/geometry.rs`, `crates/cm-vision/tests/fixtures/webcam_gt.json`
- Modify: `crates/cm-domain/src/lib.rs` (reexporta)
- Test: `crates/cm-domain/src/geometry.rs` (módulo `tests`)

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)] pub struct Rect { pub x: u32, pub y: u32, pub w: u32, pub h: u32 }`
  - `impl Rect { pub fn right(&self) -> u32; pub fn bottom(&self) -> u32; pub fn area(&self) -> u64; pub fn intersects(&self, other: &Rect) -> bool; pub fn iou(&self, other: &Rect) -> f64; pub fn contains_point(&self, x: f64, y: f64) -> bool; pub fn scaled(&self, factor: f64) -> Rect }`
  - `pub enum WebcamSource { Override, Detected, Saved }` e `pub struct WebcamDetection { pub rect: Rect, pub confidence: f64, pub source: WebcamSource }`.

- [ ] **Step 1: Write the failing tests** em `geometry.rs`: `iou` de retângulos idênticos = 1,0; disjuntos = 0,0; `Rect{0,0,10,10}` contra `Rect{5,0,10,10}` = 50/150; `intersects` é falso quando só se tocam na borda (`right() == other.x`); `scaled(0.5)` de `{10,20,100,50}` = `{5,10,50,25}`; `iou` com retângulo de área 0 = 0,0 sem dividir por zero.
- [ ] **Step 2: Run** `cargo test -p cm-domain geometry` — Expected: FAIL (tipos não existem).
- [ ] **Step 3: Implement** `Rect` e os tipos em `geometry.rs`; `iou` em `f64`.
- [ ] **Step 4: Run** `cargo test -p cm-domain` — Expected: PASS.
- [ ] **Step 5: Gabarito.** Para cada uma das 4 gravações, extrair 3 quadros de 1080p (10%, 50%, 90%), recortar a região candidata ampliada com régua em pixels (ffmpeg `crop` + `drawgrid`), ler as bordas do retângulo da webcam e escrever `webcam_gt.json` como `{"<nome sem extensão>": {"x":..,"y":..,"w":..,"h":..}}`. Desenhar cada retângulo sobre um quadro e olhar a imagem para confirmar. Registrar no ledger que o gabarito é do executor e que o Matheus o revê na Task 7.
- [ ] **Step 6: Commit** `git add -A && git commit -m "feat: Rect, tipos de detecção e gabarito da webcam"`

---

### Task 2: Amostragem de quadros em resolução cheia

**Files:**
- Create: `crates/cm-media/src/frames.rs`
- Modify: `crates/cm-media/src/lib.rs`
- Test: `crates/cm-media/tests/frames.rs` (usa `tests/common`)

**Interfaces:**
- Consumes: `CommandRunner`, `Recording`.
- Produces: `pub struct RgbFrame { pub width: u32, pub height: u32, pub time_s: f64, pub data: Vec<u8> }` (RGB24, linha a linha) e `pub fn sample_frames(runner: &dyn CommandRunner, rec: &Recording, count: usize) -> Result<Vec<RgbFrame>, MediaError>`.

- [ ] **Step 1: Write the failing tests:** (a) vídeo sintético de 20 s (`testsrc`) com `count = 8` devolve 8 quadros de `320x180` com `data.len() == 320*180*3` e `time_s` estritamente crescente, todos entre 5% e 95% da duração; (b) vídeo de 1 s com `count = 8` devolve quadros sem repetir tempo (no máximo os possíveis) e sem erro; (c) `count = 0` devolve vetor vazio sem chamar o ffmpeg (usar `FakeRunner` contador); (d) caminho com ponto inicial, espaço e acento funciona.
- [ ] **Step 2: Run** `cargo test -p cm-media --test frames` — Expected: FAIL.
- [ ] **Step 3: Implement** `sample_frames`: tempos igualmente espaçados em [5%, 95%], uma chamada `ffmpeg -v error -ss <t> -i <arquivo> -frames:v 1 -f rawvideo -pix_fmt rgb24 pipe:1` por quadro (busca por quadro-chave rápida no HDD; saída lida de `stdout`). Quadro cujo ffmpeg devolve menos bytes que `w*h*3` é descartado.
- [ ] **Step 4: Run** o teste — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "feat: amostragem de quadros em resolução cheia"`

---

### Task 3: Detector de rosto (`cm-vision`)

**Files:**
- Create: `crates/cm-vision/Cargo.toml`, `crates/cm-vision/src/lib.rs`, `crates/cm-vision/src/face.rs`, `models/face_detection_yunet_2023mar.onnx`, `crates/cm-vision/tests/face.rs`, `crates/cm-vision/tests/fixtures/{face_nov.jpg,face_jul.jpg,no_face.jpg}`
- Modify: `Cargo.toml` (membro novo; `spikes` continua excluído)

**Interfaces:**
- Consumes: `RgbFrame` (`cm-media`).
- Produces: `pub struct Face { pub rect: Rect, pub confidence: f64 }`, `pub struct FaceDetector` com `pub fn new(model: &Path) -> Result<FaceDetector, VisionError>` e `pub fn detect(&mut self, frame: &RgbFrame) -> Result<Vec<Face>, VisionError>` (já aplica NMS 0,3 e limiar 0,6; devolve em pixels da fonte); `VisionError` via `thiserror`.

- [ ] **Step 1: Write the failing tests** em `tests/face.rs`: `face_nov.jpg` e `face_jul.jpg` (quadros reais 1080p reduzidos para ≤ 150 KB, rosto na posição conhecida) devolvem exatamente 1 rosto com confiança ≥ 0,85 e centro a menos de 40 px do medido; `no_face.jpg` (quadro de jogo sem webcam) devolve 0 rostos; modelo inexistente devolve `Err`, sem pânico; imagem de 100x100 (menor que o letterbox) funciona.
- [ ] **Step 2: Run** `cargo test -p cm-vision` — Expected: FAIL.
- [ ] **Step 3: Implement** `FaceDetector` a partir de `spikes/s1-ort-yunet/src/main.rs` (`prepare` com letterbox 640 e `decode` dos mapas `cls_/obj_/bbox_` nos strides 8/16/32 + NMS), reaproveitando a sessão entre chamadas. Copiar o modelo para `models/` e conferir o sha256 da Global Constraints num teste.
- [ ] **Step 4: Run** `cargo test -p cm-vision` — Expected: PASS.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat: detector de rosto YuNet no cm-vision"`

---

### Task 4: Detector de webcam

**Files:**
- Create: `crates/cm-vision/src/webcam.rs`, `crates/cm-vision/tests/webcam.rs`, `crates/cm-vision/tests/webcam_real.rs`
- Modify: `crates/cm-vision/src/lib.rs`

**Interfaces:**
- Consumes: `RgbFrame`, `Face`, `Rect`, `WebcamDetection`.
- Produces: `pub enum WebcamOutcome { Found(WebcamDetection), NotFound { reason: String } }` e `pub fn detect_webcam(frames: &[RgbFrame], faces: &[Vec<Face>]) -> WebcamOutcome` (um vetor de rostos por quadro, mesma ordem).

Algoritmo (decisão do plano, ajustar contra o gabarito): reduzir os quadros para metade em cinza; rosto-semente = mediana dos retângulos de rosto (só quadros com rosto, ≥ 5 deles); mapa de bordas persistentes = percentil 20 do gradiente Sobel ao longo dos quadros (borda de overlay existe em todos os quadros, borda de jogo varia); se a pontuação média dos 4 lados ficar abaixo de 0,35 (sem moldura), usar o retângulo anatômico do Review Focus 1 e confiança máxima 0,7; senão, para cada um dos 4 lados, escolher a posição que maximiza a média da borda persistente ao longo do lado, dentro de uma faixa relativa ao rosto (esquerda/direita até 3 larguras de rosto, topo até 1,5, base até 2); a borda do quadro vale como lado com pontuação fixa de 0,5 do máximo; validar proporção entre 1,0 e 2,0 (largura/altura), altura do rosto entre 25% e 70% da altura do retângulo, área ≤ 25% do quadro; confiança = média das pontuações dos 4 lados normalizada por [0,1].

- [ ] **Step 1: Write the failing tests** (quadros sintéticos 960x540 com ruído de "jogo" diferente por quadro, retângulo de webcam fixo com moldura de 3 px e rosto desenhado como elipse): (a) moldura no meio do quadro, IoU ≥ 0,95 e confiança ≥ 0,6; (b) retângulo encostado no canto inferior esquerdo sem moldura nos lados do quadro, IoU ≥ 0,9; (c) rosto presente em só 3 de 12 quadros → `NotFound`; (d) rosto ocupando 70% do quadro (vídeo falado) → `NotFound` com motivo "rosto grande demais"; (e) nenhum rosto → `NotFound`; (f) mesma cena em 1280x720 e 2560x1440, IoU ≥ 0,9 (coordenadas escalam).
- [ ] **Step 2: Run** `cargo test -p cm-vision --test webcam` — Expected: FAIL.
- [ ] **Step 3: Implement** `detect_webcam` em `webcam.rs` seguindo o algoritmo acima; funções internas pequenas (`persistent_edges`, `best_side`, `validate`).
- [ ] **Step 4: Run** o teste — Expected: PASS.
- [ ] **Step 5: Gate real** em `tests/webcam_real.rs`, marcado `#[ignore]`: para cada gravação do gabarito, `probe` → `sample_frames(16)` → `FaceDetector` → `detect_webcam`; assert `Found` e `iou(gabarito) >= 0.9`; imprime IoU e confiança. Rodar `cargo test -p cm-vision --test webcam_real -- --ignored --nocapture` — Expected: 4 de 4 com IoU ≥ 0,9. Se alguma falhar, ajustar o algoritmo (não o gabarito) e registrar no ledger cada ajuste e o IoU antes e depois.
- [ ] **Step 6: Commit** `git add -A && git commit -m "feat: detector de webcam com bordas persistentes e gabarito"`

---

### Task 5: Cadeia de decisão, armazenamento e `cortes webcam`

**Files:**
- Create: `crates/cm-cli/src/webcam_cmd.rs`
- Modify: `crates/cm-store/src/{lib.rs,migrations.rs}` (schema v2), `crates/cm-cli/src/{lib.rs,main.rs,config.rs}`
- Test: `crates/cm-store/tests/store.rs`, `crates/cm-cli/tests/webcam.rs`

**Interfaces:**
- Consumes: `detect_webcam`, `sample_frames`, `FaceDetector`, `Store`, `Config`.
- Produces: `Store::save_webcam(&self, fingerprint: &str, d: &WebcamDetection, width: u32, height: u32)`, `Store::find_webcam(&self, fingerprint: &str) -> Result<Option<WebcamDetection>, StoreError>`, `Store::last_webcam_for_resolution(&self, width: u32, height: u32) -> Result<Option<Rect>, StoreError>`; `pub fn resolve_webcam(cfg: &Config, rec: &Recording, fingerprint: &str, store: &Store, detector: &mut FaceDetector) -> Result<Option<WebcamDetection>, anyhow::Error>` aplicando a cadeia da Global Constraints; `Config` ganha `webcam_overrides: HashMap<String, Rect>` (chave = nome do arquivo sem extensão) e `model_path`.

- [ ] **Step 1: Write the failing tests:** store migra v1 → v2 sem perder gravações e guarda/lê retângulo; `last_webcam_for_resolution` devolve o mais recente da mesma resolução; CLI com `config.toml` de override devolve esse retângulo com `source = Override` sem rodar detecção; CLI em vídeo sintético com webcam desenhada imprime "Webcam: x=.. y=.. w=.. h=.. confiança=.." em português; vídeo sem webcam imprime "Webcam: não encontrada, corte será só do jogo" e sai com código 0; segundo `cortes webcam` do mesmo arquivo lê do banco (não amostra de novo).
- [ ] **Step 2: Run** `cargo test -p cm-store -p cm-cli` (separados) — Expected: FAIL.
- [ ] **Step 3: Implement** migração v2 (`webcam` com `fingerprint` PK, `x,y,w,h,confidence,source,width,height`), `resolve_webcam` e o subcomando `cortes webcam <arquivo|latest>`.
- [ ] **Step 4: Run** `powershell -File scripts/check.ps1` — Expected: verde.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat: cadeia de decisão da webcam e comando cortes webcam"`

---

### Task 6: Render 9:16 (`cortes render-sample`)

**Files:**
- Create: `crates/cm-media/src/render.rs`, `crates/cm-media/tests/render.rs`, `crates/cm-cli/src/render_cmd.rs`
- Modify: `crates/cm-media/src/lib.rs`, `crates/cm-cli/src/{lib.rs,main.rs}`, `crates/cm-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `Rect`, `Recording`, `CommandRunner`.
- Produces: `pub struct Layout9x16 { pub webcam_src: Option<Rect>, pub webcam_dst_h: u32, pub game_src: Rect, pub game_dst_h: u32 }`, `pub fn plan_layout(src_w: u32, src_h: u32, webcam: Option<Rect>) -> Layout9x16` (pura), `pub fn render_9x16(runner: &dyn CommandRunner, rec: &Recording, layout: &Layout9x16, start_s: f64, dur_s: f64, out: &Path) -> Result<(), MediaError>`.

Regras de `plan_layout`: com webcam, largura do destino 1080 e `webcam_dst_h = min(round(1080*h/w), 864)` (excesso vertical cortado, centrado no rosto: `webcam_src` já devolvido ajustado); `game_dst_h = 1920 - webcam_dst_h`; `game_src` tem altura = altura da fonte e largura `src_h*1080/game_dst_h`, centrado no quadro, deslocado para a direita (ou estreitado) até não intersectar `webcam`; sem webcam, `game_dst_h = 1920` e `game_src` é o recorte central 9:16 da altura inteira.

- [ ] **Step 1: Write the failing tests** (puros, em `render.rs` e `tests/render.rs`): `plan_layout(1920,1080,Some({0,438,408,284}))` dá `webcam_dst_h = 752`, `game_dst_h = 1168`, `game_src` com `x ≥ 408` ou sem intersecção, largura ≤ 1920; webcam quadrada 400x400 dá `webcam_dst_h = 864` com `webcam_src` cortado verticalmente; webcam central `{760,400,400,300}` nunca intersecta `game_src`; `None` dá recorte 9:16 central; 1280x720 e 2560x1440 produzem o mesmo layout em razão. Teste real com ffmpeg: fonte sintética 1920x1080 com retângulo vermelho de webcam em `{0,438,408,284}` e verde no resto; `render_9x16` de 3 s gera arquivo `1080x1920`, ~90 quadros, e um quadro extraído tem faixa superior predominantemente vermelha e a inferior sem vermelho; vídeo sem áudio renderiza sem erro; caminho com acento e espaço funciona.
- [ ] **Step 2: Run** `cargo test -p cm-media --test render` — Expected: FAIL.
- [ ] **Step 3: Implement** `plan_layout` e `render_9x16` (filtergraph: `crop`+`scale` do jogo e da webcam, `vstack`, codificação e áudio da Global Constraints; áudio ausente não gera `-map` de áudio). Subcomando `cortes render-sample <arquivo|latest> [--start S] [--dur D] [--out arquivo]` que usa `resolve_webcam` e escreve o aviso "sem webcam confiável: corte só do jogo" no stdout quando cair no passo 4.
- [ ] **Step 4: Run** `powershell -File scripts/check.ps1` — Expected: verde.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat: render 9:16 com webcam em cima e comando render-sample"`

---

### Task 7: Validação em dados reais e revisão do Matheus

**Files:**
- Modify: `docs/adr/0001-arquitetura-e-linguagem.md` (seção "Portões (resultados)")

- [ ] **Step 1:** `cortes webcam` nas 4 gameplays; anotar retângulo, confiança, IoU contra o gabarito e tempo de cada uma.
- [ ] **Step 2:** `cortes render-sample --start 300 --dur 10` em cada uma; extrair 1 quadro de cada saída e inspecionar: webcam inteira em cima, sem pedaço de jogo no lugar dela e sem webcam na parte do jogo.
- [ ] **Step 3:** Publicar um artefato com os quadros 9:16, os retângulos sobre os quadros originais e campos de nota por vídeo; o Matheus confere o gabarito e dá as notas.
- [ ] **Step 4:** Registrar no ADR os IoU, tempos e ajustes; rodar `powershell -File scripts/check.ps1` — Expected: verde; commit `docs: resultados do plano 2 no ADR`.
