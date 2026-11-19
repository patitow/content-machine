use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::runner::MediaError;

const CHUNK: u64 = 1024 * 1024;

/// Impressão digital barata de um arquivo: tamanho + 1 MiB do início, do meio e do fim.
/// Não lê o arquivo inteiro (gravações de 12 GB em HDD levariam minutos).
/// Arquivos de até 3 MiB são lidos por inteiro.
pub fn fingerprint(path: &Path) -> Result<String, MediaError> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&len.to_le_bytes());

    let offsets: Vec<u64> = if len <= 3 * CHUNK {
        vec![0]
    } else {
        vec![0, len / 2 - CHUNK / 2, len - CHUNK]
    };
    let size = if len <= 3 * CHUNK { len } else { CHUNK };
    let mut buf = vec![0u8; size as usize];
    for offset in offsets {
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut buf)?;
        hasher.update(&buf);
    }
    Ok(hasher.finalize().to_hex().to_string())
}
