//! Admin authentication: one shared password, opaque bearer tokens.
//!
//! Tokens are kept in a file so admins (and the host screen) stay logged in across restarts.

use std::{path::PathBuf, sync::Mutex};

use dioxus::prelude::{HttpError, ServerFnError, StatusCode};

/// Most tokens kept; the oldest are forgotten first.
const MAX_TOKENS: usize = 50;

/// Admin password check and issued session tokens.
pub struct Auth {
    password: String,
    /// Oldest first.
    tokens: Mutex<Vec<String>>,
    /// Where tokens are saved, if anywhere.
    file: Option<PathBuf>,
    /// The password was generated at startup (no `QUAESITO_ADMIN_PASSWORD`).
    generated: bool,
}

impl Auth {
    /// Uses `QUAESITO_ADMIN_PASSWORD`, or generates a password and logs it. Tokens are saved in
    /// `file`, and the ones saved there before are still valid.
    #[must_use]
    pub fn from_env(file: PathBuf) -> Self {
        let chosen = std::env::var("QUAESITO_ADMIN_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty());
        let generated = chosen.is_none();
        let password = chosen.unwrap_or_else(|| {
            let generated = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
            dioxus::logger::tracing::warn!(
                "QUAESITO_ADMIN_PASSWORD is not set; generated admin password: {generated}"
            );
            generated
        });
        let tokens = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            password,
            tokens: Mutex::new(tokens),
            file: Some(file),
            generated,
        }
    }

    /// Tokens in memory only.
    #[must_use]
    pub fn new(password: String) -> Self {
        Self {
            password,
            tokens: Mutex::new(Vec::new()),
            file: None,
            generated: false,
        }
    }

    /// The password, when it was generated at startup and has to be shown to the admin.
    #[must_use]
    pub fn generated_password(&self) -> Option<&str> {
        self.generated.then_some(self.password.as_str())
    }

    /// Issues a token if the password matches.
    pub fn login(&self, password: &str) -> Option<String> {
        if !constant_time_eq(password.as_bytes(), self.password.as_bytes()) {
            return None;
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut tokens = self.lock();
        tokens.push(token.clone());
        let excess = tokens.len().saturating_sub(MAX_TOKENS);
        tokens.drain(..excess);
        if let Some(file) = &self.file {
            let saved = serde_json::to_vec(&*tokens)
                .map_err(std::io::Error::other)
                .and_then(|json| write_private(file, &json));
            if let Err(err) = saved {
                dioxus::logger::tracing::warn!("saving admin tokens: {err}");
            }
        }
        Some(token)
    }

    /// Whether the token was issued by [`Auth::login`].
    pub fn is_valid(&self, token: &str) -> bool {
        self.lock().iter().any(|t| t == token)
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

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Writes a file only its owner can read (on Unix): it holds login tokens.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
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

    #[test]
    fn tokens_survive_a_restart() {
        let file = std::env::temp_dir().join(format!("quaesito-tokens-{}", uuid::Uuid::new_v4()));
        let auth = Auth {
            file: Some(file.clone()),
            ..Auth::new("secret".to_owned())
        };
        let token = auth.login("secret").unwrap();
        let tokens: Vec<String> = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(tokens, [token]);
        for _ in 0..MAX_TOKENS {
            auth.login("secret").unwrap();
        }
        assert_eq!(auth.lock().len(), MAX_TOKENS);
        let _ = std::fs::remove_file(file);
    }
}
