//! Admin authentication: one shared password, opaque bearer tokens.

use std::{collections::HashSet, sync::Mutex};

use dioxus::prelude::{HttpError, ServerFnError, StatusCode};

/// Admin password check and issued session tokens.
pub struct Auth {
    password: String,
    tokens: Mutex<HashSet<String>>,
}

impl Auth {
    /// Uses `QUAESITO_ADMIN_PASSWORD`, or generates a password and logs it.
    #[must_use]
    pub fn from_env() -> Self {
        let password = std::env::var("QUAESITO_ADMIN_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| {
                let generated = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
                dioxus::logger::tracing::warn!(
                    "QUAESITO_ADMIN_PASSWORD is not set; generated admin password: {generated}"
                );
                generated
            });
        Self::new(password)
    }

    #[must_use]
    pub fn new(password: String) -> Self {
        Self {
            password,
            tokens: Mutex::new(HashSet::new()),
        }
    }

    /// Issues a token if the password matches.
    pub fn login(&self, password: &str) -> Option<String> {
        if !constant_time_eq(password.as_bytes(), self.password.as_bytes()) {
            return None;
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.lock().insert(token.clone());
        Some(token)
    }

    /// Whether the token was issued by [`Auth::login`].
    pub fn is_valid(&self, token: &str) -> bool {
        self.lock().contains(token)
    }

    /// Errors with 401 unless the token is valid.
    ///
    /// # Errors
    /// Returns an unauthorized error for unknown tokens.
    pub fn require(&self, token: &str) -> Result<(), ServerFnError> {
        if self.is_valid(token) {
            Ok(())
        } else {
            Err(HttpError::new(StatusCode::UNAUTHORIZED, "Please log in as admin again.").into())
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_issues_valid_tokens() {
        let auth = Auth::new("secret".to_owned());
        assert!(auth.login("wrong").is_none());
        assert!(auth.login("secre").is_none());
        let token = auth.login("secret").unwrap();
        assert!(auth.is_valid(&token));
        assert!(auth.require(&token).is_ok());
        assert!(auth.require("forged").is_err());
    }
}
