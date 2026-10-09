use std::collections::HashSet;
use std::fs;
use std::path::Path;

const LEGACY_IDENTIFIER: &str = "com.monocode.desktop";
const CURRENT_IDENTIFIER: &str = "com.mizius.mono";
const MIGRATION_MARKER: &str = ".mono-identity-migration-v1";

pub(crate) fn migrate() -> Result<(), String> {
    let mut roots = vec![dirs::data_dir(), dirs::data_local_dir(), dirs::config_dir()];
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        roots.push(Some(home.join("Library/Logs")));
    }

    let mut visited = HashSet::new();
    for root in roots.into_iter().flatten() {
        if !visited.insert(root.clone()) {
            continue;
        }
        migrate_directory(
            &root.join(LEGACY_IDENTIFIER),
            &root.join(CURRENT_IDENTIFIER),
        )?;
    }
    Ok(())
}

fn migrate_directory(source: &Path, destination: &Path) -> Result<(), String> {
    let source_metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", source.display())),
    };
    if !source_metadata.is_dir() || source_metadata.file_type().is_symlink() {
        return Ok(());
    }

    let marker = destination.join(MIGRATION_MARKER);
    if marker_exists(&marker)? {
        return Ok(());
    }

    if fs::symlink_metadata(destination).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Ok(());
    }

    copy_missing(source, destination)?;
    fs::write(&marker, LEGACY_IDENTIFIER).map_err(|error| format!("{}: {error}", marker.display()))
}

fn marker_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn copy_missing(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;

    for entry in fs::read_dir(source).map_err(|error| format!("{}: {error}", source.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", source.display()))?;
        let source_path = entry.path();
        let target_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", source_path.display()))?;

        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if fs::symlink_metadata(&target_path)
                .is_ok_and(|metadata| !metadata.is_dir() || metadata.file_type().is_symlink())
            {
                continue;
            }
            copy_missing(&source_path, &target_path)?;
            continue;
        }
        if !file_type.is_file() || marker_exists(&target_path)? {
            continue;
        }
        fs::copy(&source_path, &target_path)
            .map_err(|error| format!("{}: {error}", target_path.display()))?;
    }
    Ok(())
}
