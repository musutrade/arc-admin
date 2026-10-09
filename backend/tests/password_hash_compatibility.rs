use arc_admin_backend::services::auth::hash_password_async;
use argon2::{Argon2, PasswordHash, PasswordVerifier};

#[tokio::test]
async fn password_hashes_use_fresh_salts_and_verify() {
    let password = uuid::Uuid::new_v4().to_string();
    let first = hash_password_async(&password).await.expect("hash password");
    let second = hash_password_async(&password).await.expect("hash password");
    assert_ne!(first, second, "each password hash needs a fresh salt");

    for encoded in [first, second] {
        let parsed = PasswordHash::new(&encoded).expect("PHC password hash");
        assert!(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok());
        assert!(Argon2::default()
            .verify_password(b"incorrect-password", &parsed)
            .is_err());
    }
}

#[test]
fn legacy_argon2_password_hash_still_verifies() {
    // Existing Argon2 0.5-era PHC fixture from migration 0003.
    let encoded = "$argon2id$v=19$m=19456,t=2,p=1$pDDhKh46fVQNqRy3OeXTTw$+5qvGkvmKsilvsRWsskXT4k6fmmE4q35ntz6ME1UNBE";
    let parsed = PasswordHash::new(encoded).expect("legacy PHC password hash");
    assert!(Argon2::default()
        .verify_password(b"admin123", &parsed)
        .is_ok());
    assert!(Argon2::default()
        .verify_password(b"incorrect-password", &parsed)
        .is_err());
}
