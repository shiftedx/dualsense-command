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

#[test]
fn web_dist_candidates_cover_repo_and_packaged_layouts() {
    let repo = PathBuf::from("repo-root");
    let exe = PathBuf::from("install-root").join("dscc-cli");
    let candidates = web_dist_candidates(Some(&exe), Some(&repo));

    assert!(candidates.contains(&repo.join("web").join("dist")));
    assert!(candidates.contains(&PathBuf::from("install-root").join("web").join("dist")));
    assert!(candidates.contains(&PathBuf::from("install-root").join("dist")));
}
