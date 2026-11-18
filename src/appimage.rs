use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::Local;

use crate::config::Paths;
use crate::registry::InstalledApp;

#[derive(Debug, Clone)]
pub struct DiscoveredAppImage {
    pub path: PathBuf,
}

/// List every `*.AppImage` file sitting in the download/staging folder.
pub fn list_appimages(dir: &Path) -> Result<Vec<DiscoveredAppImage>> {
    let mut found = Vec::new();
    if !dir.exists() {
        return Ok(found);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("appimage") {
                    found.push(DiscoveredAppImage { path });
                }
            }
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// Metadata pulled out of the AppImage's internal `.desktop` launcher.
#[derive(Debug, Clone, Default)]
pub struct ExtractedInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub icon_path: Option<PathBuf>,
    /// Copied verbatim from the internal launcher when present. Without this,
    /// the window manager can't match the app's *running* window back to
    /// this launcher, so the taskbar/dock/alt-tab icon falls back to a
    /// generic one even though the app grid entry's icon is correct.
    pub startup_wm_class: Option<String>,
    pub mime_type: Option<String>,
    pub squashfs_root: PathBuf,
}

/// Options the "Install" flow can be run with. Advanced mode exposes all of
/// these; simple mode uses the defaults (extracted/derived name & version,
/// delete the temp extraction folder, keep the original .AppImage).
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub name: String,
    pub version: String,
    pub delete_temp_folder: bool,
    pub delete_source_appimage: bool,
}

/// Runs `--appimage-extract` inside a dated temp directory under the
/// appimages folder and parses the internal launcher for Name/Version/Icon.
pub fn extract_info(appimages_dir: &Path, appimage: &Path) -> Result<ExtractedInfo> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let temp_dir = appimages_dir.join(".tmp").join(format!("install-{stamp}"));
    fs::create_dir_all(&temp_dir)
        .with_context(|| format!("creating temp dir {}", temp_dir.display()))?;

    let abs_appimage = fs::canonicalize(appimage)
        .with_context(|| format!("resolving path for {}", appimage.display()))?;
    // Freshly downloaded AppImages (e.g. via curl) usually aren't executable yet.
    set_executable(&abs_appimage)?;

    let status = Command::new(&abs_appimage)
        .arg("--appimage-extract")
        .current_dir(&temp_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("running {} --appimage-extract", abs_appimage.display()))?;

    if !status.success() {
        bail!(
            "'{}' --appimage-extract exited with {:?}",
            abs_appimage.display(),
            status.code()
        );
    }

    let squashfs_root = temp_dir.join("squashfs-root");
    if !squashfs_root.exists() {
        bail!("extraction did not produce a squashfs-root directory");
    }

    let mut info = ExtractedInfo {
        squashfs_root: squashfs_root.clone(),
        ..Default::default()
    };

    if let Some(desktop_file) = find_desktop_file(&squashfs_root)? {
        let contents = fs::read_to_string(&desktop_file)?;
        let mut icon_key = None;
        for line in contents.lines() {
            if let Some(value) = line.strip_prefix("Name=") {
                if info.name.is_none() {
                    info.name = Some(value.trim().to_string());
                }
            } else if let Some(value) = line.strip_prefix("X-AppImage-Version=") {
                info.version = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("Icon=") {
                icon_key = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("StartupWMClass=") {
                info.startup_wm_class = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("MimeType=") {
                info.mime_type = Some(value.trim().to_string());
            }
        }

        if info.version.is_none() {
            info.version = guess_version_from_filename(appimage);
        }

        if let Some(icon_key) = icon_key {
            info.icon_path = find_icon(&squashfs_root, &icon_key);
        }
    }

    if info.name.is_none() {
        info.name = appimage
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
    }
    if info.version.is_none() {
        info.version = guess_version_from_filename(appimage);
    }

    Ok(info)
}

fn find_desktop_file(squashfs_root: &Path) -> Result<Option<PathBuf>> {
    for entry in fs::read_dir(squashfs_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn find_icon(squashfs_root: &Path, icon_key: &str) -> Option<PathBuf> {
    // 1. Per spec, a top-level "<Icon>.{png,svg,xpm}" should sit right next
    //    to the AppRun/.desktop file.
    for ext in ["png", "svg", "xpm"] {
        let candidate = squashfs_root.join(format!("{icon_key}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 2. Fallback: some AppImages skip the top-level copy and only ship the
    //    icon inside the usual hicolor/pixmaps theme dirs. Prefer the
    //    largest resolution found for the sharpest menu icon.
    if let Some(found) = find_in_theme_dirs(squashfs_root, icon_key) {
        return Some(found);
    }

    // 3. Last resort: the AppImage's own thumbnail.
    let dir_icon = squashfs_root.join(".DirIcon");
    if dir_icon.exists() {
        return Some(dir_icon);
    }
    None
}

fn find_in_theme_dirs(squashfs_root: &Path, icon_key: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    let search_roots = [
        squashfs_root.join("usr/share/icons"),
        squashfs_root.join("usr/share/pixmaps"),
    ];
    for root in search_roots {
        collect_icon_matches(&root, icon_key, &mut candidates);
    }

    // Prefer scalable (SVG) art, then the largest raster size directory
    // (e.g. ".../512x512/apps/foo.png" beats ".../48x48/apps/foo.png").
    candidates.sort_by_key(|p| std::cmp::Reverse(icon_rank(p)));
    candidates.into_iter().next()
}

fn collect_icon_matches(dir: &Path, icon_key: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_icon_matches(&path, icon_key, out);
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if stem == icon_key && matches!(ext, "png" | "svg" | "xpm") {
            out.push(path);
        }
    }
}

fn icon_rank(path: &Path) -> u32 {
    if path.extension().and_then(|e| e.to_str()) == Some("svg") {
        return u32::MAX;
    }
    // Look for a "WxH" path segment (e.g. "512x512") and rank by width.
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            if let Some((w, h)) = name.split_once('x') {
                if let (Ok(w), Ok(_h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                    return w;
                }
            }
        }
    }
    0
}

/// Overwrites every icon named `icon_key_name` found in any locally
/// installed icon theme (`~/.icons` and `~/.local/share/icons`) with our
/// own icon's content, so that whatever ends up resolving the icon by name
/// (rather than by the `.desktop` file's `Icon=<path>`, which some tray/dock
/// implementations ignore) shows the right artwork no matter which theme
/// happens to be consulted.
fn force_icon_override(icon_key_name: &str, our_icon: &Path) -> Result<()> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };
    let our_bytes = fs::read(our_icon)?;
    let our_dims = png_dimensions(&our_bytes);

    for root in [home.join(".local/share/icons"), home.join(".icons")] {
        scrub_theme_dir(&root, icon_key_name, our_icon, &our_bytes, our_dims);
    }
    Ok(())
}

fn scrub_theme_dir(
    dir: &Path,
    icon_key_name: &str,
    our_icon: &Path,
    our_bytes: &[u8],
    our_dims: Option<(u32, u32)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == our_icon {
            continue;
        }
        if path.is_dir() {
            scrub_theme_dir(&path, icon_key_name, our_icon, our_bytes, our_dims);
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.to_lowercase() != icon_key_name {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        match ext.to_lowercase().as_str() {
            "png" => {
                let _ = fs::write(&path, our_bytes);
            }
            "svg" => {
                let _ = fs::write(&path, wrap_png_as_svg(our_bytes, our_dims));
            }
            _ => {}
        }
    }
}

fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((width, height))
}

fn wrap_png_as_svg(png_bytes: &[u8], dims: Option<(u32, u32)>) -> String {
    let (w, h) = dims.unwrap_or((512, 512));
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n\
<image width=\"{w}\" height=\"{h}\" href=\"data:image/png;base64,{encoded}\"/>\n\
</svg>\n"
    )
}

/// Best-effort scan of the original filename for something that looks like
/// a version number (e.g. "desktop-3.5.343x86_64" -> "3.5.343").
fn guess_version_from_filename(appimage: &Path) -> Option<String> {
    let stem = appimage.file_stem()?.to_str()?;
    let chars: Vec<char> = stem.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                if chars[j] == '.' {
                    dots += 1;
                }
                j += 1;
            }
            if dots >= 1 {
                let candidate: String = chars[start..j].iter().collect();
                return Some(candidate.trim_matches('.').to_string());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    None
}

/// Performs the actual install: copies/moves the AppImage into the install
/// dir, copies the icon into the icon theme, writes a `.desktop` launcher and
/// records everything in the registry. Cleans up the temp extraction folder
/// (and optionally the source AppImage) according to `InstallOptions`.
pub fn install(
    paths: &Paths,
    source_appimage: &Path,
    info: &ExtractedInfo,
    opts: &InstallOptions,
) -> Result<InstalledApp> {
    let safe_name: String = opts
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let file_name = format!("{safe_name}-{}.AppImage", opts.version);
    let dest_appimage = paths.install_dir.join(&file_name);

    fs::copy(source_appimage, &dest_appimage).with_context(|| {
        format!(
            "copying {} to {}",
            source_appimage.display(),
            dest_appimage.display()
        )
    })?;
    set_executable(&dest_appimage)?;

    // The name most likely to be *guessed* by anything that resolves this
    // app's icon by name instead of respecting Icon=<path> (tray/indicator
    // extensions, some dock implementations): lowercase StartupWMClass, or
    // the app name, matching the usual icon-naming convention.
    let icon_key_name = info
        .startup_wm_class
        .clone()
        .unwrap_or_else(|| opts.name.clone())
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();

    let icon_dest = if let Some(icon_src) = &info.icon_path {
        let ext = icon_src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let icon_dir = paths.icons_dir.join("512x512/apps");
        fs::create_dir_all(&icon_dir)?;
        let dest = icon_dir.join(format!("{icon_key_name}.{ext}"));
        fs::copy(icon_src, &dest).with_context(|| {
            format!("copying icon {} to {}", icon_src.display(), dest.display())
        })?;

        // Belt-and-suspenders: some installed icon themes (e.g. Tela) ship
        // their own icon under this exact same guessed name, and whatever
        // is showing the wrong icon may be finding that instead of ours.
        // Overwrite every same-named icon found in any locally installed
        // theme so ours wins regardless of which one actually gets picked.
        if let Err(err) = force_icon_override(&icon_key_name, &dest) {
            eprintln!("warning: could not override conflicting theme icons: {err}");
        }

        Some(dest)
    } else {
        None
    };

    let desktop_path = paths.desktop_dir.join(format!("{safe_name}.desktop"));
    let icon_line = icon_dest
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "application-x-executable".to_string());

    // Carry these over verbatim from the AppImage's own internal launcher
    // when present, same as AppImageLauncher does. StartupWMClass in
    // particular is what lets the window manager match the app's *running*
    // window back to this .desktop entry; without it the taskbar/dock/
    // alt-tab icon falls back to generic even though the app grid icon
    // (Icon= above) is correct.
    let extra_lines = {
        let mut s = String::new();
        if let Some(wm_class) = &info.startup_wm_class {
            s.push_str(&format!("StartupWMClass={wm_class}\n"));
        }
        if let Some(mime_type) = &info.mime_type {
            s.push_str(&format!("MimeType={mime_type}\n"));
        }
        s
    };

    let desktop_contents = format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name={name}\n\
Comment=Installed with Universal AppImage Installer\n\
Exec=\"{exec}\" %U\n\
TryExec={exec}\n\
Icon={icon}\n\
Terminal=false\n\
Categories=Utility;\n\
{extra_lines}\
X-AppImage-Version={version}\n",
        name = opts.name,
        exec = dest_appimage.display(),
        icon = icon_line,
        version = opts.version,
    );
    fs::write(&desktop_path, desktop_contents)
        .with_context(|| format!("writing {}", desktop_path.display()))?;
    set_executable(&desktop_path)?;

    if opts.delete_temp_folder {
        if let Some(temp_dir) = info.squashfs_root.parent() {
            force_remove_dir_all(temp_dir)?;
        }
    }

    if opts.delete_source_appimage {
        let _ = fs::remove_file(source_appimage);
    }

    Ok(InstalledApp {
        name: opts.name.clone(),
        version: opts.version.clone(),
        appimage_path: dest_appimage,
        desktop_path,
        icon_path: icon_dest,
        installed_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// Recursively deletes `path`. Extracted AppImage contents (squashfs-root)
/// often contain read-only directories inherited from the squashfs image;
/// plain `fs::remove_dir_all` silently fails to unlink entries inside those
/// (unlike `rm -rf`, which fixes permissions as it goes), so we fall back to
/// forcing write permission on every directory before retrying.
fn force_remove_dir_all(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if fs::remove_dir_all(path).is_ok() {
        return Ok(());
    }
    make_writable_recursive(path)?;
    fs::remove_dir_all(path)
        .with_context(|| format!("removing {} even after fixing permissions", path.display()))
}

fn make_writable_recursive(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if path.is_dir() {
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o700);
        fs::set_permissions(path, perms)?;
        for entry in fs::read_dir(path)? {
            make_writable_recursive(&entry?.path())?;
        }
    }
    Ok(())
}

/// Extraction temp dirs (`<appimages_dir>/.tmp/install-*`) should never
/// outlive a single install attempt. Anything still there when the Install
/// screen is opened is leftover from a run that was aborted, backed out of,
/// or crashed before cleanup ran — safe to sweep away unconditionally.
pub fn cleanup_stale_temp_dirs(appimages_dir: &Path) -> Result<()> {
    force_remove_dir_all(&appimages_dir.join(".tmp"))
}

fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Removes an installed app: its `.desktop` launcher, its icon and the
/// installed `.AppImage` copy under the install dir.
pub fn uninstall(app: &InstalledApp) -> Result<()> {
    let _ = fs::remove_file(&app.appimage_path);
    let _ = fs::remove_file(&app.desktop_path);
    if let Some(icon) = &app.icon_path {
        let _ = fs::remove_file(icon);
    }
    Ok(())
}
