use std::io::Read;
use std::path::Path;

/// Compute the BLAKE3 hash of a byte slice, returning a 64-character lowercase hexadecimal string.
pub fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(data);
    hasher.finalize().to_hex().to_string()
}

/// Compute the BLAKE3 hash of any synchronous reader using a 64 KB heap-allocated buffer.
pub fn hash_reader<R: Read>(mut reader: R) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Synchronously hash a local file at `path`.
pub fn hash_file_sync(path: impl AsRef<Path>) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    hash_reader(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_hash_bytes_deterministic() {
        let data = b"Hello, AuraFS! Low-latency local-first virtual file system.";
        let hash1 = hash_bytes(data);
        let hash2 = hash_bytes(data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_hash_reader_matches_hash_bytes() {
        let data = b"Poly Platform Engineering technical showcase";
        let byte_hash = hash_bytes(data);
        let reader_hash = hash_reader(Cursor::new(data)).expect("reader hash failed");
        assert_eq!(byte_hash, reader_hash);
    }
}
