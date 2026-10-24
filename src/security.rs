//! Authentication secrets are configured by the host, never stored in world exports.
use crate::{User, bad};
use axum::http::StatusCode;
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

pub const SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
pub const MAX_SESSIONS: usize = 256;
pub const MAX_USER_SESSIONS: usize = 8;

pub struct Window {
    start: Instant,
    count: u32,
}
impl Window {
    pub fn expired(&self) -> bool {
        self.start.elapsed() >= Duration::from_secs(60)
    }
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            count: 0,
        }
    }
    pub fn allow(&mut self, limit: u32) -> bool {
        if self.start.elapsed() >= Duration::from_secs(60) {
            self.start = Instant::now();
            self.count = 0;
        }
        if self.count >= limit {
            return false;
        }
        self.count += 1;
        true
    }
}
pub struct Session {
    pub user: User,
    pub expires: Instant,
    pub actions: Window,
    pub streams: std::sync::Arc<tokio::sync::Semaphore>,
}
impl Session {
    pub fn new(user: User) -> Self {
        Self {
            user,
            expires: Instant::now() + SESSION_TTL,
            actions: Window::new(),
            streams: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}
pub fn throttled() -> (StatusCode, String) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        "Trop de requêtes. Réessayez dans une minute.".into(),
    )
}
pub fn players(raw: &str, gm: &str) -> Result<HashMap<String, String>, String> {
    let entries: HashMap<String, String> = serde_json::from_str(raw)
        .map_err(|_| "RUSTY_PLAYERS_JSON doit associer chaque nom à son secret".to_string())?;
    if entries.len() > 128 {
        return Err("Maximum de 128 joueurs configurés".into());
    }
    let mut secrets = std::collections::HashSet::new();
    for (name, secret) in &entries {
        if name.trim() != name || name.is_empty() || name.len() > 60 {
            return Err("Nom joueur invalide".into());
        }
        if secret.len() < 16 || secret == gm || !secrets.insert(secret) {
            return Err("Chaque joueur doit avoir un secret distinct d'au moins 16 octets".into());
        }
    }
    Ok(entries)
}
pub fn bind_address(raw: &str) -> Result<SocketAddr, (StatusCode, String)> {
    let addr: SocketAddr = raw
        .parse()
        .map_err(|_| bad("RUSTY_BIND doit être une adresse IP et un port"))?;
    if !addr.ip().is_loopback() {
        return Err(bad(
            "Écoute HTTP publique interdite : utiliser un proxy HTTPS vers 127.0.0.1",
        ));
    }
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_rate_window_resets_without_sleeping() {
        let mut window = Window::new();
        assert!(!window.allow(0));
        assert!(window.allow(1));
        assert!(!window.allow(1));
        window.start = Instant::now() - Duration::from_secs(61);
        assert!(window.expired());
        assert!(window.allow(1));
        assert!(!window.allow(1));
    }

    #[test]
    fn rejects_invalid_player_configuration_and_public_bindings() {
        for raw in [
            "null",
            "[]",
            "{",
            r#"{" Alice":"1234567890123456"}"#,
            r#"{"":"1234567890123456"}"#,
            r#"{"Alice":"gm-secret-12345678"}"#,
        ] {
            assert!(players(raw, "gm-secret-12345678").is_err(), "{raw}");
        }
        for addr in [
            "localhost:3000",
            "192.168.1.2:3000",
            "127.0.0.1:99999",
            "bad",
        ] {
            assert!(bind_address(addr).is_err(), "{addr}");
        }
        assert!(bind_address("[::1]:0").is_ok());
    }
    #[test]
    fn configuration_and_rate_window() {
        assert!(players(r#"{"Alice":"short"}"#, "gm").is_err());
        assert!(
            players(
                r#"{"Alice":"1234567890123456","Bob":"1234567890123456"}"#,
                "gm"
            )
            .is_err()
        );
        assert!(players(r#"{"Alice":"1234567890123456"}"#, "gm").is_ok());
        assert!(bind_address("0.0.0.0:3000").is_err());
        assert!(bind_address("[::]:3000").is_err());
        assert!(bind_address("127.0.0.1:3000").is_ok());
        let mut w = Window::new();
        for _ in 0..30 {
            assert!(w.allow(30));
        }
        assert!(!w.allow(30));
        w.start -= Duration::from_secs(61);
        assert!(w.allow(30));
    }
}
