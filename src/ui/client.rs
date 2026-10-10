//! Browser-only helpers. On the server they are inert, so components can call them freely.

/// Reads a value from `localStorage`.
#[must_use]
pub fn storage_get(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        local_storage()?.get_item(key).ok().flatten()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        None
    }
}

/// Writes a value to `localStorage`.
pub fn storage_set(key: &str, value: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (key, value);
}

/// Removes a value from `localStorage`.
pub fn storage_remove(key: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = key;
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Milliseconds since the epoch, from the browser clock.
#[must_use]
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}

/// A random number in `0..n` (always 0 on the server).
#[must_use]
pub fn random_below(n: usize) -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "small counts; Math.random() is in [0, 1)"
        )]
        let pick = (js_sys::Math::random() * n as f64) as usize;
        pick.min(n.saturating_sub(1))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = n;
        0
    }
}

/// Waits for `ms` milliseconds (never resolves on the server).
pub async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(ms).await;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ms;
        std::future::pending::<()>().await;
    }
}

/// The port the current page was loaded from (empty for the scheme default).
#[must_use]
pub fn page_port() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().port().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

/// Whether the page was opened via a loopback address, which phones cannot reach.
#[must_use]
pub fn page_is_local() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().hostname().ok())
            .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

/// The current page origin, e.g. `http://192.168.1.10:8080`.
#[must_use]
pub fn page_origin() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}
