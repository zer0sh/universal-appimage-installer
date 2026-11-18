use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Downloads `url` into `dest_dir` using the system `curl` binary, honoring
/// the server-provided filename (Content-Disposition) via `-J`. The caller
/// is expected to have already left the TUI's alternate screen so curl's own
/// progress bar (written to stderr, inherited straight through) is visible.
///
/// `--clobber` makes re-downloads (e.g. fetching a newer version of the same
/// app) overwrite the existing file — curl's default since 7.83 is to fail
/// with "File exists" when `-J`+`-O` would clobber a file, which would
/// otherwise block re-downloading anything already present.
///
/// The actual destination path is read back from curl itself via
/// `--write-out %{filename_effective}` on stdout, rather than diffing the
/// directory before/after, since diffing breaks for this exact clobber case
/// (the file is already there, so it would never look "new").
pub fn download_with_curl(url: &str, dest_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("creating {}", dest_dir.display()))?;

    let mut child = Command::new("curl")
        .current_dir(dest_dir)
        .args([
            "-L",
            "-J",
            "-O",
            "--clobber",
            "--progress-bar",
            "-w",
            "%{filename_effective}",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn curl (is it installed?)")?;

    let mut filename = String::new();
    if let Some(stdout) = child.stdout.take() {
        let mut stdout = stdout;
        stdout.read_to_string(&mut filename).ok();
    }

    let status = child.wait().context("waiting for curl to finish")?;
    if !status.success() {
        bail!("curl exited with status {:?}", status.code());
    }

    let filename = filename.trim();
    if filename.is_empty() {
        bail!("curl finished but did not report a destination filename");
    }

    Ok(dest_dir.join(filename))
}
