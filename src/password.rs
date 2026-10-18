use anyhow::{Context, anyhow};
use argon2::password_hash::phc::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash};
use tokio::task;

pub async fn hash(password: String) -> anyhow::Result<String> {
    task::spawn_blocking(move || {
        let salt = SaltString::generate();
        Ok(Argon2::default()
            .hash_password_with_salt(password.as_bytes(), &salt.as_bytes())
            .map_err(|e| anyhow!(e).context("failed to hash password"))?
            .to_string())
    })
    .await
    .context("panic in hash()")?
}

pub async fn verify(password: String, hash: String) -> anyhow::Result<bool> {
    task::spawn_blocking(move || {
        let hash = PasswordHash::new(&hash)
            .map_err(|e| anyhow!(e).context("BUG: password hash invalid"))?;

        let res = Argon2::default().verify_password(password.as_bytes(), &hash);

        match res {
            Ok(()) => Ok(true),
            Err(password_hash::Error::PasswordInvalid) => Ok(false),
            Err(e) => Err(anyhow!(e).context("failed to verify password")),
        }
    })
    .await
    .context("panic in verify()")?
}
