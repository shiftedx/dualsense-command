use super::support::*;
use super::*;

#[test]
fn web_dist_uses_configured_path_without_probing() {
    let configured = PathBuf::from("custom-web-dist");

    assert_eq!(
        web_dist_dir_from_parts(Some(configured.clone()), None, None),
        configured
    );
}

#[test]
fn web_dist_finds_packaged_assets_next_to_binary() {
    let root = temp_test_dir("dscc-web-dist");
    let exe = root.join("dscc-cli");
    let web_dist = root.join("web").join("dist");
    fs::create_dir_all(&web_dist).expect("web dist fixture directory");
    fs::write(web_dist.join("index.html"), "<!doctype html>").expect("web dist fixture");

    let found = web_dist_dir_from_parts(None, Some(&exe), Some(&root.join("other-cwd")));

    assert_eq!(found, web_dist);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn spa_fallback_serves_index_with_ok_and_keeps_api_not_found() {
    let _env = TestEnv::new(&["DSCC_WEB_DIST"]);
    let root = temp_test_dir("dscc-spa-fallback");
    fs::create_dir_all(&root).expect("web dist fixture directory");
    fs::write(
        root.join("index.html"),
        "<!doctype html><title>DSCC</title>",
    )
    .expect("web dist fixture");
    std::env::set_var("DSCC_WEB_DIST", &root);

    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/some/unknown/path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("content type header")
        .to_str()
        .unwrap()
        .to_string();
    assert!(content_type.starts_with("text/html"), "{content_type}");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8(body.to_vec()).unwrap().contains("DSCC"));

    let api_response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/unknown-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api_response.status(), StatusCode::NOT_FOUND);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn web_dist_candidates_cover_repo_and_packaged_layouts() {
    let repo = PathBuf::from("repo-root");
    let exe = PathBuf::from("install-root").join("dscc-cli");
    let candidates = web_dist_candidates(Some(&exe), Some(&repo));

    assert!(candidates.contains(&repo.join("web").join("dist")));
    assert!(candidates.contains(&PathBuf::from("install-root").join("web").join("dist")));
    assert!(candidates.contains(&PathBuf::from("install-root").join("dist")));
}
