//! Argon2id password hashes in PHC string form, computed off the async threads.

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

use crate::Error;

/// Hashes `password` with a fresh random salt.
pub(crate) async fn hash(password: String) -> Result<String, Error> {
    blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|error| Error::Password(error.to_string()))
    })
    .await
}

/// Whether `password` matches `stored`; a malformed stored hash is an error, not a mismatch.
pub(crate) async fn verify(password: String, stored: String) -> Result<bool, Error> {
    blocking(move || match Argon2::default().verify_password(password.as_bytes(), stored.as_str()) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false),
        Err(error) => Err(Error::Password(error.to_string())),
    })
    .await
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, Error> + Send + 'static) -> Result<T, Error> {
    tokio::task::spawn_blocking(work).await.map_err(|error| Error::Password(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_verify_only_their_password() {
        let stored = hash("correct horse".into()).await.unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(verify("correct horse".into(), stored.clone()).await.unwrap());
        assert!(!verify("wrong".into(), stored.clone()).await.unwrap());
        assert_ne!(hash("correct horse".into()).await.unwrap(), stored, "each hash has its own salt");
    }
}
