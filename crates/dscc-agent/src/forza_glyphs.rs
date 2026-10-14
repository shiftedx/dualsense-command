use std::{
    fs, io,
    path::{Path as FsPath, PathBuf},
};

const FORZA_HORIZON6_DEFAULT_INSTALL_PATH: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\ForzaHorizon6";
fn local_glyph_archive() -> io::Result<Vec<u8>> {
    use std::io::Read;
    let path = std::env::var_os("DSCC_FORZA_GLYPH_ARCHIVE").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Glyph packs are not bundled. Set DSCC_FORZA_GLYPH_ARCHIVE to a local ControllerIcons.zip you are entitled to use.")
    })?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 || !bytes.starts_with(b"PK\x03\x04") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Glyph archive must be a ZIP no larger than 16 MiB.",
        ));
    }
    Ok(bytes)
}

pub(crate) fn default_forza_horizon6_install_path() -> PathBuf {
    std::env::var_os("DSCC_FORZA_HORIZON6_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(FORZA_HORIZON6_DEFAULT_INSTALL_PATH))
}

pub(crate) fn resolve_forza_horizon6_install_path(path: Option<&str>) -> PathBuf {
    path.map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_forza_horizon6_install_path)
}

pub(crate) fn forza_controller_icon_targets(root: &FsPath) -> [PathBuf; 2] {
    [
        root.join("media")
            .join("UI")
            .join("Textures")
            .join("Data_Bound")
            .join("ControllerIcons.zip"),
        root.join("media")
            .join("UI")
            .join("Textures")
            .join("HiRes")
            .join("Data_Bound")
            .join("ControllerIcons.zip"),
    ]
}

pub(crate) fn forza_controller_icon_backup_path(target: &FsPath) -> PathBuf {
    target.with_extension("zip.dscc-xbox-backup")
}

pub(crate) fn trusted_forza_horizon6_install_path(
    configured_path: Option<PathBuf>,
    steam_path: Option<PathBuf>,
) -> PathBuf {
    if let Some(steam_path) = steam_path {
        return steam_path;
    }

    let default_path = default_forza_horizon6_install_path();
    if configured_path
        .as_ref()
        .and_then(|path| fs::canonicalize(path).ok())
        .zip(fs::canonicalize(&default_path).ok())
        .is_some_and(|(configured, default)| configured == default)
    {
        return configured_path.expect("configured path was checked above");
    }

    default_path
}

pub(crate) fn ensure_forza_icon_target_is_safe(root: &FsPath, target: &FsPath) -> io::Result<()> {
    if !target.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Refusing to write outside Forza Horizon 6 root: {}",
                target.display()
            ),
        ));
    }

    if let Some(mut ancestor) = target.parent() {
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "Refusing to write outside Forza Horizon 6 root: {}",
                        target.display()
                    ),
                )
            })?;
        }

        let canonical_ancestor = fs::canonicalize(ancestor)?;
        if !canonical_ancestor.starts_with(root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Refusing to follow redirected Forza glyph folder: {}",
                    ancestor.display()
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) fn install_forza_playstation_glyphs(root: PathBuf) -> io::Result<String> {
    let glyphs = local_glyph_archive()?;
    let root = canonical_forza_install_root(root)?;
    let mut backup_actions = Vec::new();
    let mut install_targets = Vec::new();

    for target in forza_controller_icon_targets(&root) {
        ensure_forza_icon_target_is_safe(&root, &target)?;
        let backup = forza_controller_icon_backup_path(&target);
        let target_exists = path_exists(&target)?;
        let backup_exists = path_exists(&backup)?;
        let target_already_playstation = file_matches_bytes(&target, &glyphs)?;
        let backup_is_playstation = file_matches_bytes(&backup, &glyphs)?;

        if backup_exists {
            if backup_is_playstation
                || (target_exists
                    && !target_already_playstation
                    && fs::read(&target)? != fs::read(&backup)?)
            {
                return Err(io::Error::new(io::ErrorKind::InvalidData,
                    "Glyph replacement identity is ambiguous. DSCC preserved the existing backup. Restore the original glyphs before switching archives; if the backup is invalid, verify the game files and preserve or remove that backup manually."));
            }
            install_targets.push(target);
            continue;
        }

        if target_exists && !target_already_playstation {
            backup_actions.push((target.clone(), backup));
            install_targets.push(target);
            continue;
        }

        if target_already_playstation {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PlayStation glyphs are already present at {}, but DSCC does not have a saved original to restore. Verify the game files once, then enable the override again.",
                    target.display()
                ),
            ));
        }

        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "ControllerIcons.zip was not found at {}. DSCC will not install PlayStation glyphs until it can save the original game file first.",
                target.display()
            ),
        ));
    }

    for (target, backup) in backup_actions {
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut source = fs::File::open(target)?;
        create_forza_original_backup(&mut source, &backup, fs::File::sync_all)?;
    }

    for target in install_targets {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = target.with_extension("zip.dscc-new");
        fs::write(&temp, &glyphs)?;
        super::persistence::replace_state_file(&temp, &target)?;
    }

    Ok(format!(
        "PlayStation button glyphs installed for Forza Horizon 6 at {}.",
        root.display()
    ))
}

pub(crate) fn create_forza_original_backup(
    source: &mut impl io::Read,
    backup: &FsPath,
    sync: impl FnOnce(&fs::File) -> io::Result<()>,
) -> io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_BACKUP: AtomicU64 = AtomicU64::new(1);
    let (staging, mut destination) = loop {
        let sequence = NEXT_BACKUP.fetch_add(1, Ordering::Relaxed);
        let mut name = backup.as_os_str().to_os_string();
        name.push(format!(".staging-{}-{sequence}", std::process::id()));
        let staging = PathBuf::from(name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
        {
            Ok(file) => break (staging, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        io::copy(source, &mut destination)?;
        sync(&destination)?;
        // Hard-link publication is exclusive: an existing original can never be
        // replaced. Restore only sees complete, synced content at the trusted
        // name; interruption beforehand can leave only an ignored staging file.
        fs::hard_link(&staging, backup)
    })();
    drop(destination);
    let _ = fs::remove_file(staging);
    result
}

pub(crate) fn restore_forza_original_glyphs(root: PathBuf) -> io::Result<String> {
    let glyphs = local_glyph_archive()?;
    let root = canonical_forza_install_root(root)?;

    let mut restore_actions = Vec::new();
    let mut invalid_backups = 0usize;
    let mut unbacked_playstation_files = Vec::new();
    for target in forza_controller_icon_targets(&root) {
        ensure_forza_icon_target_is_safe(&root, &target)?;
        let backup = forza_controller_icon_backup_path(&target);
        let backup_exists = path_exists(&backup)?;
        let backup_is_playstation = file_matches_bytes(&backup, &glyphs)?;
        let target_is_playstation = file_matches_bytes(&target, &glyphs)?;

        if backup_exists && backup_is_playstation {
            invalid_backups += 1;
            continue;
        }

        if backup_exists {
            restore_actions.push((target, backup));
            continue;
        }

        if target_is_playstation {
            unbacked_playstation_files.push(target);
        }
    }

    if invalid_backups > 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "DSCC found {invalid_backups} glyph backup file{} that already contain PlayStation icons. Verify the game files once, then enable the override again so DSCC can capture the original Xbox files.",
                if invalid_backups == 1 { "" } else { "s" }
            ),
        ));
    }

    if !unbacked_playstation_files.is_empty() {
        let target_list = unbacked_playstation_files
            .iter()
            .map(|target| target.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "DSCC found PlayStation glyph files without saved originals at {target_list}. Verify the game files once, then enable the override again so DSCC can capture the original Xbox files."
            ),
        ));
    }

    if restore_actions.is_empty() {
        return Ok(
            "Forza Horizon 6 button glyphs are already using the game defaults.".to_string(),
        );
    }

    let mut restored = 0usize;
    for (target, backup) in restore_actions {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = target.with_extension("zip.dscc-restore");
        fs::copy(&backup, &temp)?;
        super::persistence::replace_state_file(&temp, &target)?;
        restored += 1;
    }

    Ok(format!(
        "Restored {restored} original Forza Horizon 6 button glyph file{}.",
        if restored == 1 { "" } else { "s" }
    ))
}

fn file_matches_bytes(path: &FsPath, expected: &[u8]) -> io::Result<bool> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes == expected),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn path_exists(path: &FsPath) -> io::Result<bool> {
    path.try_exists()
}

fn canonical_forza_install_root(root: PathBuf) -> io::Result<PathBuf> {
    let root = fs::canonicalize(root)?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Forza Horizon 6 folder was not found at {}", root.display()),
        ))
    }
}
