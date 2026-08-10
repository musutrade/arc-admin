//! MFA cryptography and WebAuthn relying-party configuration.

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{bail, Context};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use totp_rs::{Algorithm, TOTP};
use url::Url;
use webauthn_rs::prelude::{Webauthn, WebauthnBuilder};

const NONCE_LENGTH: usize = 12;
const CIPHERTEXT_VERSION: u8 = 1;
const DEFAULT_TOTP_STEP_SECS: u64 = 30;

#[derive(Clone)]
pub struct MfaConfig {
    cipher: Aes256Gcm,
    issuer: String,
    webauthn: Webauthn,
    totp_step_secs: u64,
}

impl MfaConfig {
    pub fn new(
        encryption_key: &[u8],
        rp_id: &str,
        rp_origin: &str,
        rp_name: &str,
    ) -> anyhow::Result<Self> {
        Self::new_with_totp_step(
            encryption_key,
            rp_id,
            rp_origin,
            rp_name,
            DEFAULT_TOTP_STEP_SECS,
        )
    }

    #[doc(hidden)]
    pub fn new_with_totp_step(
        encryption_key: &[u8],
        rp_id: &str,
        rp_origin: &str,
        rp_name: &str,
        totp_step_secs: u64,
    ) -> anyhow::Result<Self> {
        if encryption_key.len() != 32 {
            bail!("MFA_ENCRYPTION_KEY must decode to exactly 32 bytes");
        }
        if totp_step_secs == 0 {
            bail!("TOTP step must be greater than zero");
        }
        let origin = Url::parse(rp_origin).context("WEBAUTHN_RP_ORIGIN must be an absolute URL")?;
        let webauthn = WebauthnBuilder::new(rp_id, &origin)
            .context("WEBAUTHN_RP_ID must be a registrable suffix of WEBAUTHN_RP_ORIGIN")?
            .rp_name(rp_name)
            .build()
            .context("invalid WebAuthn relying-party configuration")?;
        let cipher = Aes256Gcm::new_from_slice(encryption_key)
            .map_err(|_| anyhow::anyhow!("invalid MFA encryption key"))?;
        Ok(Self {
            cipher,
            issuer: rp_name.to_string(),
            webauthn,
            totp_step_secs,
        })
    }

    pub fn encrypt_totp_secret(&self, user_id: i64, secret: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut nonce_bytes = [0_u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let aad = user_id.to_be_bytes();
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: secret,
                    aad: &aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to encrypt MFA secret"))?;
        let mut encoded = Vec::with_capacity(1 + NONCE_LENGTH + ciphertext.len());
        encoded.push(CIPHERTEXT_VERSION);
        encoded.extend_from_slice(&nonce_bytes);
        encoded.extend_from_slice(&ciphertext);
        Ok(encoded)
    }

    pub fn decrypt_totp_secret(&self, user_id: i64, encoded: &[u8]) -> anyhow::Result<Vec<u8>> {
        if encoded.len() <= 1 + NONCE_LENGTH || encoded[0] != CIPHERTEXT_VERSION {
            bail!("invalid encrypted MFA secret");
        }
        let aad = user_id.to_be_bytes();
        self.cipher
            .decrypt(
                Nonce::from_slice(&encoded[1..1 + NONCE_LENGTH]),
                Payload {
                    msg: &encoded[1 + NONCE_LENGTH..],
                    aad: &aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("failed to decrypt MFA secret"))
    }

    pub fn totp(&self, account_name: &str, secret: Vec<u8>) -> anyhow::Result<TOTP> {
        TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            self.totp_step_secs,
            secret,
            Some(self.issuer.clone()),
            account_name.to_string(),
        )
        .map_err(|error| anyhow::anyhow!("failed to configure TOTP: {error}"))
    }

    pub const fn webauthn(&self) -> &Webauthn {
        &self.webauthn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_secrets_are_bound_to_the_user() {
        let config = MfaConfig::new(
            &[7_u8; 32],
            "localhost",
            "http://localhost:4200",
            "Arc Admin",
        )
        .expect("MFA config");
        let encrypted = config
            .encrypt_totp_secret(42, b"test-secret")
            .expect("encrypt");

        assert_eq!(
            config.decrypt_totp_secret(42, &encrypted).expect("decrypt"),
            b"test-secret"
        );
        assert!(config.decrypt_totp_secret(43, &encrypted).is_err());
    }
}
