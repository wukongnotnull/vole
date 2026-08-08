//! 自更新通道：检测 → 下载 → 校验 → 安装（fail-closed）。

use crate::ops::install_origin::{detect_install_layout, InstallOrigin};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tar::Archive;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

impl UpdateError {
    fn msg(s: impl Into<String>) -> Self {
        UpdateError::Message(s.into())
    }
}

#[derive(Debug, Clone)]
pub struct UpdateOptions {
    pub force: bool,
    pub nightly: bool,
    pub check_only: bool,
    pub yes: bool,
    pub current_version: String,
    pub binary_path: PathBuf,
    pub config_dir: PathBuf,
    pub confirm_brew_self_update: bool,
    pub repo: String,
    pub arch_triple: Option<String>,
}

impl UpdateOptions {
    pub fn arch(&self) -> String {
        self.arch_triple.clone().unwrap_or_else(host_arch_triple)
    }
}

fn host_arch_triple() -> String {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64-apple-darwin".into(),
        "x86_64" => "x86_64-apple-darwin".into(),
        other => format!("{other}-apple-darwin"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    AlreadyLatest {
        version: String,
    },
    Check {
        current: String,
        latest: Option<String>,
        origin: InstallOrigin,
        channel: String,
    },
    Updated {
        version: String,
    },
    BrewPreferred,
    NightlyBrewRejected,
    Failed(String),
}

pub trait UpdateTransport {
    fn latest_stable_tag(&self) -> Result<String, UpdateError>;
    fn latest_main_commit(&self) -> Result<String, UpdateError>;
    fn download(&self, url: &str, dest: &Path) -> Result<(), UpdateError>;
}

pub trait VersionProbe {
    fn version_of(&self, binary: &Path) -> Result<String, UpdateError>;
}

pub struct ExecVersionProbe;

impl VersionProbe for ExecVersionProbe {
    fn version_of(&self, binary: &Path) -> Result<String, UpdateError> {
        let out = Command::new(binary)
            .arg("--version")
            .output()
            .map_err(|e| UpdateError::msg(format!("run --version: {e}")))?;
        if !out.status.success() {
            return Err(UpdateError::msg(format!(
                "--version failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        parse_version_token(text.lines().next().unwrap_or(""))
            .ok_or_else(|| UpdateError::msg(format!("unable to parse --version output: {text}")))
    }
}

fn parse_version_token(line: &str) -> Option<String> {
    line.split_whitespace().last().map(|s| s.to_string())
}

#[derive(Default)]
pub struct FakeUpdateTransport {
    pub latest_tag: String,
    pub latest_commit: String,
    pub files: HashMap<String, Vec<u8>>,
    pub download_calls: Mutex<usize>,
}

impl FakeUpdateTransport {
    pub fn new(latest_tag: impl Into<String>) -> Self {
        Self {
            latest_tag: latest_tag.into(),
            latest_commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            files: HashMap::new(),
            download_calls: Mutex::new(0),
        }
    }

    pub fn with_file(mut self, url: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.files.insert(url.into(), bytes);
        self
    }
}

impl UpdateTransport for FakeUpdateTransport {
    fn latest_stable_tag(&self) -> Result<String, UpdateError> {
        if self.latest_tag.is_empty() {
            return Err(UpdateError::msg("unable to resolve latest tag"));
        }
        Ok(self.latest_tag.trim_start_matches('v').to_string())
    }

    fn latest_main_commit(&self) -> Result<String, UpdateError> {
        if self.latest_commit.is_empty() {
            return Err(UpdateError::msg("unable to resolve main commit"));
        }
        Ok(self.latest_commit.clone())
    }

    fn download(&self, url: &str, dest: &Path) -> Result<(), UpdateError> {
        *self.download_calls.lock().unwrap() += 1;
        let bytes = self
            .files
            .get(url)
            .ok_or_else(|| UpdateError::msg(format!("fake missing url: {url}")))?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(dest, bytes)?;
        Ok(())
    }
}

pub struct CurlUpdateTransport {
    pub repo: String,
}

impl CurlUpdateTransport {
    pub fn new(repo: impl Into<String>) -> Self {
        Self { repo: repo.into() }
    }
}

impl UpdateTransport for CurlUpdateTransport {
    fn latest_stable_tag(&self) -> Result<String, UpdateError> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);
        let body = curl_to_string(&url)?;
        let tag = body
            .lines()
            .find_map(|l| {
                let t = l.trim();
                if let Some(rest) = t.strip_prefix("\"tag_name\":") {
                    let v = rest.trim().trim_matches(',').trim().trim_matches('"');
                    return Some(v.trim_start_matches('v').to_string());
                }
                // also handle compact JSON
                None
            })
            .or_else(|| {
                // serde-less scrape: "tag_name":"v1.2.3"
                let key = "\"tag_name\"";
                let idx = body.find(key)?;
                let after = &body[idx + key.len()..];
                let colon = after.find(':')?;
                let rest = after[colon + 1..].trim_start();
                let rest = rest.strip_prefix('"')?;
                let end = rest.find('"')?;
                Some(rest[..end].trim_start_matches('v').to_string())
            })
            .ok_or_else(|| UpdateError::msg("unable to parse latest release tag"))?;
        Ok(tag)
    }

    fn latest_main_commit(&self) -> Result<String, UpdateError> {
        let url = format!("https://api.github.com/repos/{}/commits/main", self.repo);
        let body = curl_to_string(&url)?;
        let key = "\"sha\"";
        let idx = body
            .find(key)
            .ok_or_else(|| UpdateError::msg("unable to parse commit sha"))?;
        let after = &body[idx + key.len()..];
        let colon = after
            .find(':')
            .ok_or_else(|| UpdateError::msg("unable to parse commit sha"))?;
        let rest = after[colon + 1..].trim_start();
        let rest = rest
            .strip_prefix('"')
            .ok_or_else(|| UpdateError::msg("unable to parse commit sha"))?;
        let end = rest
            .find('"')
            .ok_or_else(|| UpdateError::msg("unable to parse commit sha"))?;
        let sha = &rest[..end];
        if sha.len() < 7 {
            return Err(UpdateError::msg("commit sha too short"));
        }
        Ok(sha.to_string())
    }

    fn download(&self, url: &str, dest: &Path) -> Result<(), UpdateError> {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let status = Command::new("curl")
            .args([
                "-fsSL",
                "--connect-timeout",
                "10",
                "--max-time",
                "120",
                "-o",
            ])
            .arg(dest)
            .arg(url)
            .status()
            .map_err(|e| UpdateError::msg(format!("curl: {e}")))?;
        if !status.success() {
            return Err(UpdateError::msg(format!(
                "download failed ({status}): {url}"
            )));
        }
        Ok(())
    }
}

fn curl_to_string(url: &str) -> Result<String, UpdateError> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: vole-update",
            url,
        ])
        .output()
        .map_err(|e| UpdateError::msg(format!("curl: {e}")))?;
    if !out.status.success() {
        return Err(UpdateError::msg(format!(
            "curl failed for {url}: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn run_update(
    opts: &UpdateOptions,
    transport: &dyn UpdateTransport,
    version_probe: &dyn VersionProbe,
) -> Result<UpdateOutcome, UpdateError> {
    let layout = detect_install_layout(&opts.binary_path, Some(&opts.config_dir));
    let channel = read_channel(&opts.config_dir);

    if opts.nightly && layout.origin == InstallOrigin::Homebrew {
        return Ok(UpdateOutcome::NightlyBrewRejected);
    }

    if opts.check_only {
        let latest = if opts.nightly {
            transport
                .latest_main_commit()
                .ok()
                .map(|s| s.chars().take(7).collect::<String>())
        } else {
            transport.latest_stable_tag().ok()
        };
        return Ok(UpdateOutcome::Check {
            current: opts.current_version.clone(),
            latest,
            origin: layout.origin,
            channel,
        });
    }

    let brew_blocked = layout.origin == InstallOrigin::Homebrew
        && !opts.force
        && !opts.yes
        && !opts.confirm_brew_self_update;
    if brew_blocked {
        return Ok(UpdateOutcome::BrewPreferred);
    }

    if opts.nightly {
        return run_nightly(opts, &layout.prefix_bin, transport, version_probe);
    }

    run_stable(opts, &layout.prefix_bin, transport, version_probe)
}

fn run_stable(
    opts: &UpdateOptions,
    prefix_bin: &Path,
    transport: &dyn UpdateTransport,
    version_probe: &dyn VersionProbe,
) -> Result<UpdateOutcome, UpdateError> {
    let latest = transport.latest_stable_tag()?;
    if !opts.force && versions_equal(&opts.current_version, &latest) {
        return Ok(UpdateOutcome::AlreadyLatest {
            version: opts.current_version.clone(),
        });
    }

    let arch = opts.arch();
    let asset = format!("vole-{latest}-{arch}.tar.gz");
    let base = format!(
        "https://github.com/{}/releases/download/v{latest}",
        opts.repo
    );
    let tarball_url = format!("{base}/{asset}");
    let sums_url = format!("{base}/SHA256SUMS");

    let tmp = tempfile_dir("vole-update")?;
    let tarball_path = tmp.join(&asset);
    let sums_path = tmp.join("SHA256SUMS");

    transport
        .download(&sums_url, &sums_path)
        .map_err(|e| UpdateError::msg(format!("SHA256SUMS download failed (fail-closed): {e}")))?;
    let sums_text = fs::read_to_string(&sums_path)?;
    transport.download(&tarball_url, &tarball_path)?;

    verify_sha256(&tarball_path, &sums_text, &asset)?;

    install_tarball(&tarball_path, prefix_bin)?;
    let installed = prefix_bin.join("vole");
    let reported = version_probe.version_of(&installed)?;
    if !versions_equal(&reported, &latest) {
        return Err(UpdateError::msg(format!(
            "post-install version mismatch: got {reported}, want {latest}"
        )));
    }

    write_install_channel(&opts.config_dir, "stable", None)?;
    Ok(UpdateOutcome::Updated { version: latest })
}

fn run_nightly(
    opts: &UpdateOptions,
    prefix_bin: &Path,
    transport: &dyn UpdateTransport,
    version_probe: &dyn VersionProbe,
) -> Result<UpdateOutcome, UpdateError> {
    let commit = transport.latest_main_commit()?;
    let short = commit.chars().take(7).collect::<String>();
    let installed_commit = read_commit_hash(&opts.config_dir);
    if !opts.force {
        if let Some(cur) = installed_commit {
            if cur.chars().take(7).collect::<String>() == short {
                return Ok(UpdateOutcome::AlreadyLatest {
                    version: format!("nightly-{short}"),
                });
            }
        }
    }

    // Explicit --nightly may install an attested nightly tarball when transport provides one;
    // checksum is required if SHA256SUMS is present, otherwise source-style payload is allowed
    // only because the user passed --nightly (never as stable fallback).
    let arch = opts.arch();
    let asset = format!("vole-nightly-{arch}.tar.gz");
    let tarball_url = format!(
        "https://github.com/{}/releases/download/nightly/{asset}",
        opts.repo
    );
    let sums_url = format!(
        "https://github.com/{}/releases/download/nightly/SHA256SUMS",
        opts.repo
    );

    let tmp = tempfile_dir("vole-update-nightly")?;
    let tarball_path = tmp.join(&asset);
    let sums_path = tmp.join("SHA256SUMS");

    transport.download(&tarball_url, &tarball_path)?;
    if transport.download(&sums_url, &sums_path).is_ok() {
        let sums_text = fs::read_to_string(&sums_path)?;
        verify_sha256(&tarball_path, &sums_text, &asset)?;
    }

    install_tarball(&tarball_path, prefix_bin)?;
    // Nightly success: binary must run --version (version number may equal workspace).
    let _ = version_probe.version_of(&prefix_bin.join("vole"))?;
    write_install_channel(&opts.config_dir, "nightly", Some(&commit))?;
    Ok(UpdateOutcome::Updated {
        version: format!("nightly-{short}"),
    })
}

pub fn verify_sha256(file: &Path, sums_text: &str, asset_name: &str) -> Result<(), UpdateError> {
    let expected = extract_checksum(sums_text, asset_name).ok_or_else(|| {
        UpdateError::msg(format!(
            "checksum missing for asset {asset_name} (fail-closed)"
        ))
    })?;
    let actual = file_sha256_hex(file)?;
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(UpdateError::msg(format!(
            "checksum mismatch for {asset_name}: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn extract_checksum(sums_text: &str, asset_name: &str) -> Option<String> {
    for line in sums_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?;
        if name == asset_name || name.ends_with(&format!("/{asset_name}")) {
            return Some(hash.to_string());
        }
    }
    None
}

fn file_sha256_hex(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn install_tarball(tarball: &Path, prefix_bin: &Path) -> Result<(), UpdateError> {
    let file = File::open(tarball)?;
    let dec = GzDecoder::new(file);
    let mut archive = Archive::new(dec);
    let staging = tempfile_dir("vole-update-stage")?;
    archive
        .unpack(&staging)
        .map_err(|e| UpdateError::msg(format!("untar: {e}")))?;

    let src_bin = find_installed_vole(&staging)?
        .ok_or_else(|| UpdateError::msg("tarball missing bin/vole"))?;
    fs::create_dir_all(prefix_bin)?;
    let dest = prefix_bin.join("vole");
    let dest_tmp = prefix_bin.join(".vole.update.tmp");
    fs::copy(&src_bin, &dest_tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest_tmp, perms)?;
    }
    fs::rename(&dest_tmp, &dest)?;

    if let Some(rules) = find_rules_dir(&staging) {
        let share = prefix_bin
            .parent()
            .unwrap_or(prefix_bin)
            .join("share/vole/rules");
        if let Ok(entries) = fs::read_dir(&rules) {
            let _ = fs::create_dir_all(&share);
            for ent in entries.flatten() {
                let path = ent.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    let _ = fs::copy(&path, share.join(ent.file_name()));
                }
            }
        }
    }
    Ok(())
}

fn find_installed_vole(root: &Path) -> Result<Option<PathBuf>, UpdateError> {
    let direct = root.join("bin/vole");
    if direct.is_file() {
        return Ok(Some(direct));
    }
    for ent in fs::read_dir(root)? {
        let ent = ent?;
        let candidate = ent.path().join("bin/vole");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn find_rules_dir(root: &Path) -> Option<PathBuf> {
    let direct = root.join("share/vole/rules");
    if direct.is_dir() {
        return Some(direct);
    }
    if let Ok(entries) = fs::read_dir(root) {
        for ent in entries.flatten() {
            let candidate = ent.path().join("share/vole/rules");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

fn versions_equal(a: &str, b: &str) -> bool {
    a.trim_start_matches('v') == b.trim_start_matches('v')
}

fn tempfile_dir(prefix: &str) -> Result<PathBuf, UpdateError> {
    let base = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&base)?;
    Ok(base)
}

fn channel_file(config_dir: &Path) -> PathBuf {
    config_dir.join("install_channel")
}

fn read_channel(config_dir: &Path) -> String {
    let path = channel_file(config_dir);
    let Ok(text) = fs::read_to_string(path) else {
        return "stable".into();
    };
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("CHANNEL=") {
            let v = v.trim();
            if matches!(v, "stable" | "nightly" | "dev") {
                return v.to_string();
            }
        }
    }
    "stable".into()
}

fn read_commit_hash(config_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(channel_file(config_dir)).ok()?;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("COMMIT_HASH=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn write_install_channel(
    config_dir: &Path,
    channel: &str,
    commit: Option<&str>,
) -> Result<(), UpdateError> {
    fs::create_dir_all(config_dir)?;
    let mut body = format!("CHANNEL={channel}\n");
    if let Some(c) = commit {
        body.push_str(&format!("COMMIT_HASH={c}\n"));
    }
    let path = channel_file(config_dir);
    let mut f = File::create(path)?;
    f.write_all(body.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tar::Builder;

    fn make_tarball(version: &str, arch: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let enc = GzEncoder::new(&mut buf, Compression::default());
            let mut ar = Builder::new(enc);
            let script = format!("#!/bin/sh\necho \"vole {version}\"\n");
            let mut header = tar::Header::new_gnu();
            header.set_size(script.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            let path = format!("vole-{version}-{arch}/bin/vole");
            ar.append_data(&mut header, path, script.as_bytes())
                .unwrap();
            ar.finish().unwrap();
        }
        buf
    }

    fn sha256_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn manual_opts(dir: &Path, ver: &str) -> (UpdateOptions, PathBuf) {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("vole");
        fs::write(&exe, b"#!/bin/sh\necho vole old\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(&exe).unwrap().permissions();
            p.set_mode(0o755);
            fs::set_permissions(&exe, p).unwrap();
        }
        let opts = UpdateOptions {
            force: false,
            nightly: false,
            check_only: false,
            yes: true,
            current_version: ver.into(),
            binary_path: exe.clone(),
            config_dir: dir.join("config"),
            confirm_brew_self_update: false,
            repo: "wukongnotnull/vole".into(),
            arch_triple: Some("aarch64-apple-darwin".into()),
        };
        (opts, exe)
    }

    #[test]
    fn checksum_mismatch_is_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (mut opts, _) = manual_opts(dir.path(), "2.3.0");
        opts.force = true;
        let arch = "aarch64-apple-darwin";
        let latest = "9.9.9";
        let tar_bytes = make_tarball(latest, arch);
        let asset = format!("vole-{latest}-{arch}.tar.gz");
        let sums =
            format!("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef  {asset}\n");
        let base = format!("https://github.com/wukongnotnull/vole/releases/download/v{latest}");
        let transport = FakeUpdateTransport::new(latest)
            .with_file(format!("{base}/{asset}"), tar_bytes)
            .with_file(format!("{base}/SHA256SUMS"), sums.into_bytes());
        let err = run_update(&opts, &transport, &ExecVersionProbe).unwrap_err();
        assert!(
            format!("{err}").to_ascii_lowercase().contains("checksum"),
            "{err}"
        );
        assert_eq!(*transport.download_calls.lock().unwrap(), 2);
    }

    #[test]
    fn brew_without_force_prefers_brew() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/vole/2.3.0/bin");
        fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("vole");
        fs::write(&real, b"x").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let link = bin.join("vole");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let opts = UpdateOptions {
            force: false,
            nightly: false,
            check_only: false,
            yes: false,
            current_version: "2.3.0".into(),
            binary_path: link,
            config_dir: dir.path().join("config"),
            confirm_brew_self_update: false,
            repo: "wukongnotnull/vole".into(),
            arch_triple: Some("aarch64-apple-darwin".into()),
        };
        let transport = FakeUpdateTransport::new("9.9.9");
        let out = run_update(&opts, &transport, &ExecVersionProbe).unwrap();
        assert_eq!(out, UpdateOutcome::BrewPreferred);
        assert_eq!(*transport.download_calls.lock().unwrap(), 0);
    }

    #[test]
    fn stable_update_installs_when_checksum_matches() {
        let dir = tempfile::tempdir().unwrap();
        let (mut opts, _) = manual_opts(dir.path(), "2.3.0");
        let arch = "aarch64-apple-darwin";
        let latest = "9.9.9";
        let tar_bytes = make_tarball(latest, arch);
        let digest = sha256_bytes(&tar_bytes);
        let asset = format!("vole-{latest}-{arch}.tar.gz");
        let sums = format!("{digest}  {asset}\n");
        let base = format!("https://github.com/wukongnotnull/vole/releases/download/v{latest}");
        let transport = FakeUpdateTransport::new(latest)
            .with_file(format!("{base}/{asset}"), tar_bytes)
            .with_file(format!("{base}/SHA256SUMS"), sums.into_bytes());
        let out = run_update(&opts, &transport, &ExecVersionProbe).unwrap();
        assert_eq!(
            out,
            UpdateOutcome::Updated {
                version: latest.into()
            }
        );
        let ver = ExecVersionProbe
            .version_of(&dir.path().join("bin/vole"))
            .unwrap();
        assert_eq!(ver, latest);
        let _ = &mut opts;
    }

    #[test]
    fn sums_missing_asset_is_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (mut opts, _) = manual_opts(dir.path(), "2.3.0");
        opts.force = true;
        let arch = "aarch64-apple-darwin";
        let latest = "9.9.9";
        let tar_bytes = make_tarball(latest, arch);
        let asset = format!("vole-{latest}-{arch}.tar.gz");
        let sums = "abcd  other-file.tar.gz\n";
        let base = format!("https://github.com/wukongnotnull/vole/releases/download/v{latest}");
        let transport = FakeUpdateTransport::new(latest)
            .with_file(format!("{base}/{asset}"), tar_bytes)
            .with_file(format!("{base}/SHA256SUMS"), sums.as_bytes().to_vec());
        let err = run_update(&opts, &transport, &ExecVersionProbe).unwrap_err();
        assert!(format!("{err}").contains("checksum"), "{err}");
    }

    #[test]
    fn nightly_rejected_on_homebrew() {
        let dir = tempfile::tempdir().unwrap();
        let cellar = dir.path().join("Cellar/vole/2.3.0/bin");
        fs::create_dir_all(&cellar).unwrap();
        let real = cellar.join("vole");
        fs::write(&real, b"x").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let link = bin.join("vole");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let opts = UpdateOptions {
            force: true,
            nightly: true,
            check_only: false,
            yes: true,
            current_version: "2.3.0".into(),
            binary_path: link,
            config_dir: dir.path().join("config"),
            confirm_brew_self_update: true,
            repo: "wukongnotnull/vole".into(),
            arch_triple: Some("aarch64-apple-darwin".into()),
        };
        let out = run_update(&opts, &FakeUpdateTransport::new("9.9.9"), &ExecVersionProbe).unwrap();
        assert_eq!(out, UpdateOutcome::NightlyBrewRejected);
    }

    #[test]
    fn already_latest_skips_download() {
        let dir = tempfile::tempdir().unwrap();
        let (opts, _) = manual_opts(dir.path(), "9.9.9");
        let transport = FakeUpdateTransport::new("9.9.9");
        let out = run_update(&opts, &transport, &ExecVersionProbe).unwrap();
        assert_eq!(
            out,
            UpdateOutcome::AlreadyLatest {
                version: "9.9.9".into()
            }
        );
        assert_eq!(*transport.download_calls.lock().unwrap(), 0);
    }
}
