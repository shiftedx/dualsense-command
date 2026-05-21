use std::net::SocketAddr;

use crate::{env_policy, AppSettings, PersistedAgentState, PersistenceStore};

pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:43473";
pub const DEFAULT_FORZA_BIND_ADDR: &str = "127.0.0.1:5300";
pub const FORZA_BIND_ADDR_ENV: &str = "DSCC_FORZA_BIND_ADDR";
pub const FORZA_LAN_ENABLE_ENV: &str = "DSCC_ENABLE_LAN_FORZA";
pub const LAN_API_ENABLE_ENV: &str = "DSCC_ENABLE_LAN_API";

pub(crate) fn default_agent_bind_addr() -> SocketAddr {
    DEFAULT_BIND_ADDR
        .parse()
        .expect("static DSCC bind address is valid")
}

fn all_interfaces_agent_bind_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], port))
}

pub(crate) fn lan_api_enabled() -> bool {
    env_policy::flag_enabled(LAN_API_ENABLE_ENV)
}

pub(crate) fn desired_agent_bind_addr(settings: &AppSettings, port: u16) -> SocketAddr {
    if settings.listen_on_all_interfaces && lan_api_enabled() {
        all_interfaces_agent_bind_addr(port)
    } else {
        SocketAddr::from(([127, 0, 0, 1], port))
    }
}

pub fn resolve_agent_bind_addr() -> SocketAddr {
    if let Ok(value) = std::env::var("DSCC_AGENT_ADDR") {
        if let Ok(addr) = value.trim().parse::<SocketAddr>() {
            if addr.ip().is_loopback() || lan_api_enabled() {
                return addr;
            }
            tracing::warn!(
                bind_addr = %addr,
                opt_in = LAN_API_ENABLE_ENV,
                "ignoring non-loopback DSCC_AGENT_ADDR without explicit LAN API opt-in"
            );
        }
    }

    let default = default_agent_bind_addr();
    let Some(store) = PersistenceStore::default() else {
        return default;
    };
    match store.load().map(PersistedAgentState::normalized) {
        Ok(state) if state.app_settings.listen_on_all_interfaces && lan_api_enabled() => {
            all_interfaces_agent_bind_addr(default.port())
        }
        _ => default,
    }
}

pub(crate) fn resolve_forza_bind_addr() -> SocketAddr {
    let default: SocketAddr = DEFAULT_FORZA_BIND_ADDR
        .parse()
        .expect("static Forza loopback bind address is valid");
    match std::env::var(FORZA_BIND_ADDR_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return default;
            }
            match trimmed.parse::<SocketAddr>() {
                Ok(addr) => {
                    if !addr.ip().is_loopback() {
                        if !env_policy::flag_enabled(FORZA_LAN_ENABLE_ENV) {
                            tracing::warn!(
                                bind_addr = %addr,
                                opt_in = FORZA_LAN_ENABLE_ENV,
                                "ignoring non-loopback Forza Data Out bind address without explicit LAN opt-in"
                            );
                            return default;
                        }
                        tracing::warn!(
                            bind_addr = %addr,
                            "Forza Data Out listener is bound to a non-loopback address; ensure your firewall is configured intentionally"
                        );
                    }
                    addr
                }
                Err(error) => {
                    tracing::warn!(
                        env = FORZA_BIND_ADDR_ENV,
                        value = trimmed,
                        %error,
                        "Could not parse Forza bind override; falling back to default loopback bind"
                    );
                    default
                }
            }
        }
        Err(_) => default,
    }
}
