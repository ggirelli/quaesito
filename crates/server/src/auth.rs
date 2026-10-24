//! Admin authentication: one shared password, opaque bearer tokens.
//!
//! Tokens are kept in a file so admins (and the host screen) stay logged in across restarts, and
//! expire [`LOGIN_LIFETIME`] after logging in.

use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dioxus::prelude::{HttpError, ServerFnError, StatusCode};
use quaesito_core::constant_time_eq;
use serde::{Deserialize, Serialize};

/// Most tokens kept; the oldest are forgotten first.
const MAX_TOKENS: usize = 50;

/// How long a login lasts: a stolen or forgotten one stops working by the next day.
pub const LOGIN_LIFETIME: Duration = Duration::from_hours(12);

/// An issued token.
#[derive(Serialize, Deserialize)]
struct Login {
    token: String,
    /// When it was issued, in seconds since 1970.
    issued: u64,
}

impl Login {
    fn is_current(&self, now: u64) -> bool {
        now.saturating_sub(self.issued) < LOGIN_LIFETIME.as_secs()
    }
}

/// Now, in seconds since 1970.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Admin password check and issued session tokens.
pub struct Auth {
    password: String,
    /// Oldest first.
    tokens: Mutex<Vec<Login>>,
    /// Where tokens are saved, if anywhere.
    file: Option<PathBuf>,
    /// The password was generated at startup (no `QUAESITO_ADMIN_PASSWORD`).
    generated: bool,
}

impl Auth {
    /// Uses `QUAESITO_ADMIN_PASSWORD`, or generates a password and logs it. Tokens are saved in
    /// `file`, and the ones saved there before stay valid until they expire (tokens saved by
    /// versions without expiry are dropped).
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
        let now = now();
        let mut tokens = self.lock();
        tokens.retain(|login| login.is_current(now));
        tokens.push(Login {
            token: token.clone(),
            issued: now,
        });
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

    /// Whether the token was issued by [`Auth::login`] and has not expired.
    pub fn is_valid(&self, token: &str) -> bool {
        let now = now();
        self.lock().iter().any(|login| {
            constant_time_eq(login.token.as_bytes(), token.as_bytes()) && login.is_current(now)
        })
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

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Login>> {
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
    fn tokens_expire() {
        let auth = Auth::new("secret".to_owned());
        let fresh = auth.login("secret").unwrap();
        auth.lock().push(Login {
            token: "old".to_owned(),
            issued: now() - LOGIN_LIFETIME.as_secs(),
        });
        assert!(auth.is_valid(&fresh));
        assert!(!auth.is_valid("old"));
        // Logging in again forgets expired tokens.
        auth.login("secret").unwrap();
        assert!(auth.lock().iter().all(|login| login.token != "old"));
    }

    #[test]
    fn tokens_survive_a_restart() {
        let file = std::env::temp_dir().join(format!("quaesito-tokens-{}", uuid::Uuid::new_v4()));
        let auth = Auth {
            file: Some(file.clone()),
            ..Auth::new("secret".to_owned())
        };
        let token = auth.login("secret").unwrap();
        let tokens: Vec<Login> = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, token);
        for _ in 0..MAX_TOKENS {
            auth.login("secret").unwrap();
        }
        assert_eq!(auth.lock().len(), MAX_TOKENS);
        let _ = std::fs::remove_file(file);
    }
}
