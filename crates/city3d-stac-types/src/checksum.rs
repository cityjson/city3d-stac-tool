//! Content hashing for the STAC File extension.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Compute the checksum of a file for the STAC File Extension `file:checksum` field.
///
/// Returns a hex-encoded [Multihash](https://github.com/multiformats/multihash) of the
/// file's SHA-256 digest: the bytes `0x12` (sha2-256 function code) and `0x20` (32-byte
/// digest length) followed by the digest itself. The file is read in chunks so memory
/// usage stays constant regardless of file size. Returns `None` if the file cannot be read.
pub fn file_checksum(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();

    // Multihash prefix: 0x12 = sha2-256, 0x20 = 32-byte length.
    let mut multihash = Vec::with_capacity(2 + digest.len());
    multihash.push(0x12);
    multihash.push(0x20);
    multihash.extend_from_slice(&digest);
    Some(hex::encode(multihash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_checksum_is_sha256_multihash() {
        // Write a file with known content and verify the multihash-encoded SHA-256.
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello").unwrap();
        temp_file.flush().unwrap();

        let checksum = file_checksum(temp_file.path()).unwrap();

        // SHA-256("hello") prefixed with multihash header 0x12 (sha2-256) 0x20 (len 32)
        assert_eq!(
            checksum,
            "12202cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_file_checksum_missing_file_is_none() {
        let path = Path::new("/nonexistent/path/to/file.json");
        assert!(file_checksum(path).is_none());
    }
}
