use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub(crate) fn request_origin_matches_host(headers: &HeaderMap) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
    else {
        return true;
    };
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|host| !host.is_empty())
    else {
        return false;
    };
    let Some(origin_host) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .and_then(|origin| origin.split('/').next())
    else {
        return false;
    };

    origin_host.eq_ignore_ascii_case(host)
}

pub(crate) async fn reject_cross_origin_mutations(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) || request_origin_matches_host(request.headers())
    {
        return Ok(next.run(request).await);
    }

    Err(StatusCode::FORBIDDEN)
}
