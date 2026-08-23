pub mod google_docs;
pub mod local;
pub mod notion;

use crate::error::AppResult;
use crate::models::DocumentRecord;
use base64::Engine;
use sha2::{Digest, Sha256};

pub trait SourceConnector {
    fn scan(&self) -> AppResult<Vec<DocumentRecord>>;
}

pub fn hash_text(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!(
        "sha256:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    )
}

#[cfg(test)]
mod tests {
    use super::hash_text;

    #[test]
    fn content_hash_is_stable_sha256() {
        assert_eq!(
            hash_text("abc"),
            "sha256:ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
        );
    }
}
