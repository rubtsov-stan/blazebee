#[cfg(feature = "encryption")]
pub mod encryption {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use rand::Rng;

    const NONCE_SIZE: usize = 12;
    const KEY_SIZE: usize = 32;

    pub struct EncryptionManager {
        master_key: [u8; KEY_SIZE],
    }

    impl EncryptionManager {
        pub fn new(master_key: [u8; KEY_SIZE]) -> Self {
            Self { master_key }
        }

        pub fn generate_random_key() -> [u8; KEY_SIZE] {
            let mut rng = rand::thread_rng();
            let mut key = [0u8; KEY_SIZE];
            rng.fill(&mut key);
            key
        }

        pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
            let cipher = Aes256Gcm::new(self.master_key.as_ref().into());
            let mut rng = rand::thread_rng();
            let mut nonce_bytes = [0u8; NONCE_SIZE];
            rng.fill(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            let mut ciphertext = nonce_bytes.to_vec();

            match cipher.encrypt(nonce, plaintext) {
                Ok(encrypted) => {
                    ciphertext.extend_from_slice(&encrypted);
                    Ok(ciphertext)
                }
                Err(e) => Err(format!("Encryption failed: {}", e)),
            }
        }

        pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
            if ciphertext.len() < NONCE_SIZE {
                return Err("Ciphertext too short".to_string());
            }

            let cipher = Aes256Gcm::new(self.master_key.as_ref().into());
            let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
            let encrypted_data = &ciphertext[NONCE_SIZE..];

            cipher
                .decrypt(nonce, encrypted_data)
                .map_err(|e| format!("Decryption failed: {}", e))
        }
    }
}
