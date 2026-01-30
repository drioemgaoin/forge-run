use sha2::{Digest, Sha256};

pub fn generate_api_key() -> (String, String, String) {
    let raw = format!("frk_{}", uuid::Uuid::new_v4());
    let prefix = raw.chars().take(8).collect::<String>();
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = hex::encode(hasher.finalize());
    (raw, prefix, hash)
}
