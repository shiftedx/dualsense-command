use super::support::*;
use super::*;

const TEST_GLYPHS: &[u8] = b"PK\x03\x04original-dscc-test-fixture";
fn glyph_archive_env() -> TestEnv {
    let env = TestEnv::new(&["DSCC_FORZA_GLYPH_ARCHIVE"]);
    let path = std::env::temp_dir().join(format!("dscc-glyph-fixture-{}.zip", std::process::id()));
    fs::write(&path, TEST_GLYPHS).expect("local archive fixture");
    std::env::set_var("DSCC_FORZA_GLYPH_ARCHIVE", path);
    env
}

#[test]
fn forza_glyph_installer_requires_an_explicit_local_archive() {
    let _env = TestEnv::new(&["DSCC_FORZA_GLYPH_ARCHIVE"]);
    std::env::remove_var("DSCC_FORZA_GLYPH_ARCHIVE");
    let error = install_forza_playstation_glyphs(PathBuf::from("unused"))
        .expect_err("no embedded glyph pack");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(error.to_string().contains("DSCC_FORZA_GLYPH_ARCHIVE"));
}

#[test]
fn forza_glyph_installer_rejects_invalid_and_oversized_local_archives() {
    let _env = glyph_archive_env();
    let path = std::env::var_os("DSCC_FORZA_GLYPH_ARCHIVE").unwrap();
    for contents in [
        b"not a zip".to_vec(),
        [b"PK\x03\x04".as_slice(), &vec![0; 16 * 1024 * 1024]].concat(),
    ] {
        fs::write(&path, contents).unwrap();
        let error = install_forza_playstation_glyphs(PathBuf::from("unused"))
            .expect_err("archive must be bounded and recognizable");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn forza_trusted_install_path_ignores_untrusted_configured_path_without_steam_catalog() {
    let _env = TestEnv::new(&["DSCC_FORZA_HORIZON6_INSTALL_DIR"]);
    let default_root = temp_test_dir("dscc-forza-default-root");
    let configured_root = temp_test_dir("dscc-forza-configured-root");
    fs::create_dir_all(&default_root).expect("default root fixture");
    fs::create_dir_all(&configured_root).expect("configured root fixture");
    std::env::set_var("DSCC_FORZA_HORIZON6_INSTALL_DIR", &default_root);

    let trusted = trusted_forza_horizon6_install_path(Some(configured_root.clone()), None);

    assert_eq!(
        fs::canonicalize(trusted).expect("trusted path canonicalizes"),
        fs::canonicalize(&default_root).expect("default path canonicalizes")
    );
    let _ = fs::remove_dir_all(default_root);
    let _ = fs::remove_dir_all(configured_root);
}

#[test]
fn forza_trusted_install_path_prefers_discovered_steam_path() {
    let _env = TestEnv::new(&["DSCC_FORZA_HORIZON6_INSTALL_DIR"]);
    let default_root = temp_test_dir("dscc-forza-default-root");
    let steam_root = temp_test_dir("dscc-forza-steam-root");
    fs::create_dir_all(&default_root).expect("default root fixture");
    fs::create_dir_all(&steam_root).expect("steam root fixture");
    std::env::set_var("DSCC_FORZA_HORIZON6_INSTALL_DIR", &default_root);

    let trusted = trusted_forza_horizon6_install_path(None, Some(steam_root.clone()));

    assert_eq!(trusted, steam_root);
    let _ = fs::remove_dir_all(default_root);
    let _ = fs::remove_dir_all(steam_root);
}

#[test]
fn forza_icon_target_guard_rejects_paths_outside_install_root() {
    let root = temp_test_dir("dscc-forza-safe-root");
    let outside_root = temp_test_dir("dscc-forza-outside-root");
    fs::create_dir_all(&root).expect("root fixture");
    fs::create_dir_all(&outside_root).expect("outside fixture");
    let outside_target = outside_root.join("ControllerIcons.zip");

    let error = ensure_forza_icon_target_is_safe(&root, &outside_target)
        .expect_err("outside target should be rejected");

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside_root);
}

#[test]
fn forza_glyph_installer_backs_up_and_restores_controller_icons() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!("dscc-forza-glyph-test-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp glyph test dir should be removable");
    }

    let targets = forza_controller_icon_targets(&root);
    for (index, target) in targets.iter().enumerate() {
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be creatable");
        fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
    }

    install_forza_playstation_glyphs(root.clone()).expect("glyph install should succeed");
    for (index, target) in targets.iter().enumerate() {
        assert_eq!(
            fs::read(target).expect("installed icon should be readable"),
            TEST_GLYPHS
        );
        assert!(
            forza_controller_icon_backup_path(target).exists(),
            "original icon should be backed up"
        );
        assert_eq!(
            fs::read_to_string(forza_controller_icon_backup_path(target))
                .expect("backup icon should be readable"),
            format!("xbox-icons-{index}")
        );
    }

    restore_forza_original_glyphs(root.clone()).expect("glyph restore should succeed");
    for (index, target) in forza_controller_icon_targets(&root).iter().enumerate() {
        assert_eq!(
            fs::read_to_string(target).expect("restored icon should be readable"),
            format!("xbox-icons-{index}")
        );
    }

    fs::remove_dir_all(&root).expect("temp glyph test dir should be removable");
}

#[test]
fn forza_glyph_installer_refuses_to_install_without_originals() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!(
        "dscc-forza-glyph-missing-originals-test-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp missing originals dir should be removable");
    }
    fs::create_dir_all(&root).expect("temp missing originals root should be creatable");

    let error = install_forza_playstation_glyphs(root.clone())
        .expect_err("glyph install should refuse missing original icon files");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);

    for target in forza_controller_icon_targets(&root) {
        assert!(
            !target.exists(),
            "installer should not create unbacked PlayStation icon files"
        );
        assert!(
            !forza_controller_icon_backup_path(&target).exists(),
            "installer should not create backups when originals are missing"
        );
    }

    fs::remove_dir_all(&root).expect("temp missing originals dir should be removable");
}

#[test]
fn forza_glyph_installer_recovers_bad_playstation_backups_after_verify() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!(
        "dscc-forza-glyph-recovery-test-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp glyph recovery dir should be removable");
    }

    let targets = forza_controller_icon_targets(&root);
    for (index, target) in targets.iter().enumerate() {
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be creatable");
        fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
        fs::write(forza_controller_icon_backup_path(target), TEST_GLYPHS)
            .expect("stale PlayStation backup should be writable");
    }

    install_forza_playstation_glyphs(root.clone()).expect("glyph install should succeed");
    restore_forza_original_glyphs(root.clone()).expect("glyph restore should succeed");

    for (index, target) in targets.iter().enumerate() {
        assert_eq!(
            fs::read_to_string(target).expect("restored icon should be readable"),
            format!("xbox-icons-{index}")
        );
    }

    fs::remove_dir_all(&root).expect("temp glyph recovery dir should be removable");
}

#[test]
fn forza_glyph_restore_succeeds_when_defaults_are_already_present() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!(
        "dscc-forza-glyph-defaults-test-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp defaults dir should be removable");
    }

    let targets = forza_controller_icon_targets(&root);
    for (index, target) in targets.iter().enumerate() {
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be creatable");
        fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
    }

    let message = restore_forza_original_glyphs(root.clone())
        .expect("restore should no-op when defaults are present");
    assert!(
        message.contains("already using the game defaults"),
        "restore should report a successful no-op"
    );
    for (index, target) in targets.iter().enumerate() {
        assert_eq!(
            fs::read_to_string(target).expect("default icon should remain readable"),
            format!("xbox-icons-{index}")
        );
    }

    fs::remove_dir_all(&root).expect("temp defaults dir should be removable");
}

#[test]
fn forza_glyph_restore_refuses_unbacked_playstation_files() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!(
        "dscc-forza-glyph-unbacked-test-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp unbacked dir should be removable");
    }

    let targets = forza_controller_icon_targets(&root);
    for target in &targets {
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be creatable");
        fs::write(target, TEST_GLYPHS).expect("PlayStation icon should be writable");
    }

    let error = restore_forza_original_glyphs(root.clone())
        .expect_err("restore should refuse PlayStation icons without backups");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    for target in &targets {
        assert_eq!(
            fs::read(target).expect("PlayStation icon should remain readable"),
            TEST_GLYPHS
        );
    }

    fs::remove_dir_all(&root).expect("temp unbacked dir should be removable");
}

#[test]
fn forza_glyph_restore_validates_every_target_before_replacing_files() {
    let _env = glyph_archive_env();
    let root = std::env::temp_dir().join(format!(
        "dscc-forza-glyph-partial-restore-test-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp partial restore dir should be removable");
    }

    let targets = forza_controller_icon_targets(&root);
    for target in &targets {
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be creatable");
        fs::write(target, TEST_GLYPHS).expect("PlayStation icon should be writable");
    }
    fs::write(
        forza_controller_icon_backup_path(&targets[0]),
        "xbox-icons-restorable",
    )
    .expect("backup icon should be writable");

    let error = restore_forza_original_glyphs(root.clone())
        .expect_err("restore should refuse a partial restore with one unbacked target");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    for target in &targets {
        assert_eq!(
            fs::read(target).expect("PlayStation icon should remain readable"),
            TEST_GLYPHS
        );
    }

    fs::remove_dir_all(&root).expect("temp partial restore dir should be removable");
}
