# content-machine: design

Data: 2026-11-18. Status: **aguardando aprovação** (nada foi construído).

## Intenção

Matheus grava e transmite pelo OBS (gameplay com webcam, e vídeo falado). Quer transformar essas gravações em cortes **horizontais (16:9) e verticais (9:16)** com a maior qualidade possível, rodando **100% local**, sem depender da infraestrutura de terceiros, e com IA **opcional e trocável** (assinatura do Claude, API compatível com OpenAI, modelo local).

Por que um repositório próprio: as ferramentas de código aberto existentes (OpenShorts, SupoClip, AutoClip) escolhem trechos pela transcrição, não têm prova independente de qualidade e não tratam gameplay com pouca fala. A literatura (áudio + vídeo + chat) mostra que sinais combinados funcionam melhor que só áudio.

## Objetivos

- Dada uma gravação, sugerir os melhores momentos e renderizar cortes 16:9 e 9:16 prontos para postar.
- Qualidade **medida**, não presumida: gabarito feito pelo Matheus nas gameplays reais e métricas objetivas.
- Funcionar **sem IA**. A IA só reordena candidatos e escreve título e legenda.
- Interface única de IA com três adaptadores: `claude` em modo headless, API compatível com OpenAI, modelo local (Ollama ou llama.cpp).
- CLI como interface principal; servidor MCP fino por cima, expondo os mesmos comandos.
- Acabamento: cortar em fronteiras de frase, normalizar volume, legendas, encode NVENC.

## Não objetivos

- Live vertical (o Matheus não pretende por enquanto). Por isso **Aitum Vertical fica de fora**.
- Postar automaticamente nas redes.
- Interface gráfica. Editor de vídeo completo.
- Suporte a vários jogos no começo.

## Anti-objetivos (falha mesmo que "funcione")

- Depender de um serviço de nuvem, conta ou chave para o caminho básico.
- Cortes que começam ou terminam no meio de uma frase.
- Resultado que o Matheus não consegue verificar contra o próprio gabarito.

## Restrições

- Windows 11, RTX 3060 (NVENC), ffmpeg instalado, Python 3.14.5 (versão nova demais para várias bibliotecas de ML; ver "Fica para o construtor").
- Gravações em `E:\.VIDEOS GRAVADOS\OBS` (82 arquivos, 95 GB, MKV HEVC 1080p60 ou 30). Todas as antigas têm **uma só faixa de áudio** (mix).
- Repositórios do Matheus ficam em `C:\Users\msoli\projetos\<nome>`. Este: `C:\Users\msoli\projetos\content-machine` (local indicado e autorizado pelo Matheus).
- Segredos (API keys) nunca em texto aberto no repositório.

## Abordagens consideradas

1. **Fork do OpenShorts (9k stars, MIT, MCP).** Mais rápido para ter algo. Mas herda a seleção por transcrição, que é o ponto fraco em gameplay, e uma base grande para entender e manter. Não resolve o que o Matheus pediu (controle total).
2. **Pipeline próprio em camadas de sinais (escolhida).** FFmpeg + Python, sinais de áudio/movimento/fala, IA só no fim. Dá controle e medição. Custa mais para construir.
3. **Híbrido: próprio, com OpenShorts só como referência de comparação.** É o que a escolhida já inclui no teste cego; não é uma terceira arquitetura.

O Matheus escolheu construir o próprio (2).

## Arquitetura

```
gravação(ões) → ingest → sinais → candidatos → (IA opcional) → render → saída + manifesto
```

1. **ingest:** `ffprobe`; detecta faixas de áudio (Mix/Jogo/Comunicação/Mic) e, se existirem, arquivos separados do Source Record (webcam, jogo, mic).
2. **sinais** (cada um gera uma série temporal normalizada, em módulos separados):
   - energia/picos do áudio do **jogo** (faixa 2, ou mix nas gravações antigas; a faixa 3, Comunicação, tem as vozes do Discord e pode servir de sinal de reação);
   - **fala e exclamação no microfone** (faster-whisper local; faixa 4 ou mix);
   - movimento e mudança de cena (FFmpeg);
3. **candidatos:** funde os sinais com pesos configuráveis, cria janelas com margem antes e depois, junta janelas próximas, ajusta início e fim para fronteira de frase (a partir da transcrição).
4. **IA (opcional):** recebe candidatos + transcrição + resumo dos sinais; devolve ordem, título e legenda. Se não houver IA configurada, usa só a pontuação dos sinais.
5. **render:** 16:9 direto da fonte; 9:16 empilhado (webcam em cima, jogo embaixo) ou jogo recortado com webcam menor. Com Source Record, a composição usa os arquivos separados; sem ele, usa um retângulo de webcam configurado por sessão. Legendas queimadas, loudnorm, NVENC.
6. **saída:** pasta por gravação com cortes e `manifest.json` (janela, pontuação, sinais que dispararam, título).

**Módulo opcional por jogo (depois):** detecção de kill feed por OCR para **Battlefield 6** (jogo indicado pelo Matheus, "acho que é bf6", ainda não confirmado). Entra atrás da mesma interface de sinal, só depois de a base funcionar.

### Interface de IA

Uma interface com um método: dados os candidatos e a transcrição, devolve JSON estruturado (ordem, título, legenda, justificativa curta). Adaptadores: `claude` (CLI headless, assinatura), OpenAI-compatível (URL base + chave por variável de ambiente), local (Ollama / llama.cpp via API OpenAI-compatível). Qualquer um é substituível sem tocar nos sinais.

### Interfaces externas

- **CLI:** `cortes analyze | suggest | render | eval`.
- **MCP:** servidor fino com as mesmas operações.

## Como garantir qualidade

1. **Gabarito:** o Matheus marca os melhores momentos nas 4 gameplays de teste (cerca de 96 min, ~15 a 25 por sessão): `2025-11-30 13-54-24`, `2025-11-30 14-45-52`, `2026-07-27 22-26-47`, `2026-07-27 23-05-35`. As reuniões e aulas servem de teste de "conteúdo falado".
2. **Métricas:** recall (momentos marcados que foram encontrados, com tolerância de janela) e precisão nos K primeiros cortes sugeridos. Relatório gerado pelo `cortes eval`.
3. **Teste cego** contra o OpenShorts nas mesmas gravações: o Matheus nota os cortes sem saber a origem.
4. **Ablação:** medir o ganho de cada sinal (áudio, fala, movimento, IA) para saber o que realmente ajuda.
5. **Gravar melhor daqui para frente:** 4 faixas de áudio e Source Record na webcam (feitos).

## Setup do OBS

- **Feito (2026-11-18):** perfil "Padrão" em 1080p**60**, faixa de cor Limited, HEVC CQP 19 (perfil main), gravando **4 faixas de áudio**: 1 Mix, 2 Jogo (dispositivo padrão do Windows), 3 Comunicação (dispositivo de comunicação do Windows, o Chat do fone), 4 Mic. Faixas separadas permitem reduzir ou aumentar cada uma na pós-edição (ex.: abaixar o jogo quando o Matheus fala, ou em cutscene).
- **Feito:** Source Record 0.5.0 instalado, com um filtro **só na webcam** (arquivo separado, encoder H.264 leve). Filtro no jogo removido para evitar 3 encodes simultâneos.
- **Feito:** Aitum Multistream desativado no OBS (desinstalação exigiria administrador).
- **Não haverá marcador por atalho:** o fluxo é 100% automático, pós-gravação.
- **Pendente (Matheus, a cada gravação):** apontar a captura de jogo para o BF6.
- **Observação:** o dispositivo padrão de reprodução do Windows está no monitor (LS27DG30X), não no fone Game. O OBS segue o padrão atual do Windows para a faixa 2.

## Decisões do Matheus

- Construir o próprio, local, com IA plugável.
- Sem live vertical agora, logo sem Aitum.
- **Tudo automático, sem apertar botão para marcar momentos.**
- Áudio em faixas separadas: padrão (jogo) e comunicação, para ajustar volumes depois.
- Acabamento (frases, volume, legendas, NVENC) e interface de IA com os três adaptadores: aprovados.
- Gameplays de teste em `E:\.VIDEOS GRAVADOS\OBS`.

## Decisões minhas (para o Matheus conferir)

- Python + FFmpeg, como CLI com MCP por cima.
- Nome do pacote/CLI: `cortes` (o repositório se chama `content-machine`).
- Sem IA no caminho básico; IA só reordena e escreve texto.
- Kill feed de BF6 só depois da base.
- Não fiz a revisão por subagente do documento, para economizar uso; revisei eu mesmo.

## Fica para o construtor

- **Versão do Python:** 3.14 provavelmente não tem pacotes de `faster-whisper`/`ctranslate2`/`mediapipe`; usar 3.12 em ambiente isolado (uv ou venv).
- Pesos iniciais dos sinais e limiares (calibrar com o gabarito).
- Escolha do modelo Whisper (equilíbrio velocidade e precisão na 3060).
- Estratégia de detecção/configuração do retângulo da webcam nas gravações antigas.
- Formato do arquivo de gabarito e da ferramenta de marcação (sugestão: página local para marcar em vídeo acelerado).
- Estilo das legendas e do layout 9:16.

## Riscos

- Sem o gabarito do Matheus, não há medida de qualidade. É o gargalo humano do projeto.
- Gravações antigas só têm mix de áudio, então o sinal "jogo" fica mais fraco nelas.
- O formato exato das chamadas do `claude` headless pode mudar; isolar no adaptador.
