# Verificação completa: fmt, clippy e testes. Para no primeiro erro.
$ErrorActionPreference = 'Stop'
$env:Path += ";$env:USERPROFILE\.cargo\bin"
cargo fmt --check;                                                if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets -- -D warnings;            if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo test --workspace;                                           if ($LASTEXITCODE) { exit $LASTEXITCODE }
