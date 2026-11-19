# content-machine

Pipeline local de cortes 16:9/9:16 a partir de gravações do OBS. Decisões em `docs/adr/0001-arquitetura-e-linguagem.md`; requisitos em `docs/superpowers/specs/`; planos em `docs/superpowers/plans/`.

## Comandos
- Verificar tudo: `powershell -File scripts/check.ps1` (fmt, clippy `-D warnings`, testes). Deve passar a cada commit.
- Rust fixo em `rust-toolchain.toml`. O `cargo` fica em `%USERPROFILE%\.cargo\bin`.

## Regras
- Contratos antes de implementação: tipos e assinaturas primeiro, testes antes do código (TDD).
- Código em inglês; textos para o usuário (CLI, erros mostrados) em português.
- Cache de proxies em `E:\.VIDEOS GRAVADOS\content-machine-cache` (o C: está quase cheio).
- Pasta do OBS: `E:\.VIDEOS GRAVADOS\OBS` (ponto inicial, espaço e acento: sempre testar).
- Faixas OBS: `Mix`, `Jogo (padrao)`, `Comunicacao`, `Mic`; gravações antigas têm 1 faixa `Track1` (= Mix); há vídeos sem áudio.
- Ingest lê o arquivo uma única vez (HDD ~97 MB/s). Nunca ler arquivo inteiro para hash.
- Nenhum caminho pede entrada interativa. UI: React, também no navegador, operável por agente (`aria-label`, `data-testid`); nada exclusivo do Electron.
- Commits sem `Co-Authored-By` e sem selo de IA. Branch de feature a partir de `main`.

## Ordem dos planos
1 fundação (este) → 2 webcam + render → 3 sinais + avaliação → 4 worker Whisper → 5 daemon + API + MCP + skill `gerar-cortes` → 6 UI React + Electron.
