use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use autolight_core::cache::cache_entry_matches_payload;
use autolight_core::project::{AudioAsset, CacheEntry};

use super::audio::inspect_wav_file;

pub(super) fn path_from_qml(path: &str) -> PathBuf {
    let value = path.trim();
    let path = if let Some(rest) = value.strip_prefix("file://") {
        if rest.starts_with('/') {
            rest
        } else {
            let decoded = percent_decode(rest);
            return PathBuf::from(format!("//{decoded}"));
        }
    } else {
        value
    };
    let decoded = percent_decode(path);
    PathBuf::from(strip_windows_drive_url_slash(&decoded))
}

pub(super) fn strip_windows_drive_url_slash(path: &str) -> &str {
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        &path[1..]
    } else {
        path
    }
}

pub(super) fn current_project_dir(path: &str) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    path_from_qml(path).parent().map(Path::to_path_buf)
}

pub(super) fn audio_asset_load_error(asset: &AudioAsset) -> Option<String> {
    let path = Path::new(&asset.path);
    audio_asset_load_error_at_path(asset, path, &asset.path)
}

pub(super) fn audio_asset_load_error_at_path(
    asset: &AudioAsset,
    path: &Path,
    display_path: &str,
) -> Option<String> {
    match inspect_wav_file(path) {
        Ok(inspection) => {
            let metadata_changed = (asset.duration - inspection.metadata.duration).abs() > 1e-9
                || asset.sample_rate != inspection.metadata.sample_rate
                || asset.channels != inspection.metadata.channels;
            if (!asset.fingerprint.is_empty() && asset.fingerprint != inspection.fingerprint)
                || metadata_changed
            {
                Some(format!("input audio asset modified: {display_path}"))
            } else {
                None
            }
        }
        Err(_) if !path.is_file() => Some(format!("input audio asset offline: {display_path}")),
        Err(_) => Some(format!("input audio asset modified: {display_path}")),
    }
}

pub(super) fn audio_asset_project_dir_relink_path(
    asset: &AudioAsset,
    project_dir: Option<&Path>,
) -> Option<PathBuf> {
    let project_dir = project_dir?;
    let mut file_names = Vec::default();
    if !asset.relink_hint.is_empty() {
        if let Some(file_name) = Path::new(&asset.relink_hint)
            .file_name()
            .and_then(|name| name.to_str())
        {
            file_names.push(file_name.to_string());
        }
    }
    if let Some(file_name) = Path::new(&asset.path)
        .file_name()
        .and_then(|name| name.to_str())
    {
        if !file_names.iter().any(|candidate| candidate == file_name) {
            file_names.push(file_name.to_string());
        }
    }

    file_names.into_iter().find_map(|file_name| {
        let candidate = project_dir.join(file_name);
        let display_path = candidate.to_string_lossy();
        if audio_asset_load_error_at_path(asset, &candidate, &display_path).is_none() {
            Some(candidate)
        } else {
            None
        }
    })
}

pub(super) fn cache_entry_is_valid(entry: &CacheEntry, project_dir: Option<&Path>) -> bool {
    if !cache_entry_path_is_safe(Path::new(&entry.path)) {
        return false;
    }

    let Some(path) = cache_entry_path(entry, project_dir) else {
        return true;
    };
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if metadata.len() != entry.size_bytes {
        return false;
    }

    let mut payload = Vec::with_capacity(entry.size_bytes as usize);
    if file.read_to_end(&mut payload).is_err() {
        return false;
    }
    cache_entry_matches_payload(entry, &payload).unwrap_or(false)
}

pub(super) fn cache_entry_path(entry: &CacheEntry, project_dir: Option<&Path>) -> Option<PathBuf> {
    let entry_path = Path::new(&entry.path);
    if entry_path.is_absolute() {
        None
    } else {
        project_dir.map(|directory| directory.join(entry_path))
    }
}

pub(super) fn cache_entry_path_is_safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

pub(super) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).to_string()
}

pub(super) fn with_autolight_suffix(path: PathBuf) -> PathBuf {
    if path.extension().and_then(|suffix| suffix.to_str()) == Some("autolight") {
        return path;
    }
    path.with_extension("autolight")
}
