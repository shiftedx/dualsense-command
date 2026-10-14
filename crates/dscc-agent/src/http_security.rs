use axum::{
    body::Body,
    extract::State,
    http::{header, uri::Authority, HeaderMap, Method, Request, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};

use crate::AgentState;

fn host_authority(headers: &HeaderMap) -> Option<Authority> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())?;
    let authority: Authority = host.parse().ok()?;
    let suffix = authority.as_str().strip_prefix(authority.host())?;
    if host.contains('@')
        || (!suffix.is_empty()
            && !suffix.strip_prefix(':').is_some_and(|port| {
                !port.is_empty()
                    && port.bytes().all(|b| b.is_ascii_digit())
                    && port.parse::<u16>().is_ok()
            }))
    {
        return None;
    }
    Some(authority)
}

fn request_host_is_allowed(headers: &HeaderMap, bind_addr: SocketAddr) -> bool {
    if !headers.contains_key(header::HOST) {
        return !headers.contains_key(header::ORIGIN);
    }
    let Some(authority) = host_authority(headers) else {
        return false;
    };
    if !bind_addr.ip().is_loopback() {
        // Explicit LAN binding retains custom hostname/reverse-proxy access.
        return true;
    }
    let host = authority.host();
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip == bind_addr.ip())
}

pub(crate) fn request_origin_matches_host(headers: &HeaderMap, bind_addr: SocketAddr) -> bool {
    if !request_host_is_allowed(headers, bind_addr) {
        return false;
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(origin) = origin
        .to_str()
        .ok()
        .and_then(|value| value.parse::<Uri>().ok())
    else {
        return false;
    };
    if !matches!(origin.scheme_str(), Some("http" | "https"))
        || origin
            .path_and_query()
            .is_some_and(|path| path.as_str() != "/")
    {
        return false;
    }
    let Some(origin_host) = origin.authority() else {
        return false;
    };
    let Some(host) = host_authority(headers) else {
        return false;
    };

    origin_host.as_str().eq_ignore_ascii_case(host.as_str())
}

pub(crate) async fn reject_cross_origin_mutations(
    State(state): State<AgentState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !request_host_is_allowed(request.headers(), state.bind_addr) {
        return Err(StatusCode::FORBIDDEN);
    }
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) || request_origin_matches_host(request.headers(), state.bind_addr)
    {
        return Ok(next.run(request).await);
    }

    Err(StatusCode::FORBIDDEN)
}
