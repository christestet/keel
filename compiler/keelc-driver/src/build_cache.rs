//! Cross-invocation up-to-date check for `keel build` (KDR-0019 incremental
//! budget). A clean, successful build records a stamp file beside the output
//! binary; the next `keel build` with identical inputs verifies the stamp and
//! the binary's contents and skips the frontend and `go build` entirely —
//! the same no-op-rebuild contract `go build` itself provides.
//!
//! Scope: whole-build cutoff only. An edited source still pays the full
//! pipeline; per-module reuse across invocations needs persistent query
//! state (a future KDR). The stamp hashes the single source file because a
//! module is a file today — when multi-file packages land, the stamp must
//! cover the package fileset.
//!
//! Determinism (AGENTS.md hard rule 7): the cutoff never rewrites the binary
//! (trivially byte-identical) and a stamp is only written for builds that
//! emitted zero diagnostics, so a skipped build prints exactly what a real
//! rebuild would have printed.

use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

/// Inputs that must match for a previous build's binary to still be correct.
/// `content` is the stamp file's body up to (not including) the `binary` line.
pub(crate) struct BuildStamp {
    stamp_path: PathBuf,
    binary_path: PathBuf,
    content: String,
}

impl BuildStamp {
    /// Gather the build inputs. Returns `None` when any identity source is
    /// unavailable (no `go` on PATH, unreadable own executable): the build
    /// then simply proceeds uncached and any real problem surfaces with its
    /// own diagnostic.
    pub(crate) fn compute(source_path: &Path, text: &str, milestone: u32) -> Option<BuildStamp> {
        let binary_path = output_binary_path(source_path);
        let stamp_path = stamp_path_for(&binary_path)?;
        let compiler = compiler_identity()?;
        let go = go_version()?;
        let content = format!(
            "keelstamp v1\ncompiler {compiler}\ngo {go}\nmilestone {milestone}\nsource {} {:016x}\n",
            text.len(),
            hash_bytes(text.as_bytes()),
        );
        Some(BuildStamp {
            stamp_path,
            binary_path,
            content,
        })
    }

    /// True when the stamp on disk matches these inputs and the recorded
    /// binary is present and unmodified. Any I/O error or mismatch means
    /// "rebuild"; this check must never fail a build.
    pub(crate) fn is_up_to_date(&self) -> bool {
        let Ok(stamp) = fs::read_to_string(&self.stamp_path) else {
            return false;
        };
        let Some(binary_line) = stamp.strip_prefix(&self.content) else {
            return false;
        };
        let Ok(binary) = fs::read(&self.binary_path) else {
            return false;
        };
        binary_line == binary_record(&binary)
    }

    /// Record the just-built binary. Best-effort: a failed write only means
    /// the next build runs in full.
    pub(crate) fn record(&self) {
        let Ok(binary) = fs::read(&self.binary_path) else {
            return;
        };
        let stamp = format!("{}{}", self.content, binary_record(&binary));
        let _ = fs::write(&self.stamp_path, stamp);
    }
}

/// Where `keel build <source>` places the binary: source file stem (plus the
/// platform suffix) in the source's directory, canonicalized so `-o` is
/// correct even when `go build` runs from the temp module dir.
pub(crate) fn output_binary_path(source_path: &Path) -> PathBuf {
    let binary_name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("keel-out");
    let binary_name = format!("{binary_name}{}", env::consts::EXE_SUFFIX);
    let output_dir = source_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::canonicalize(output_dir)
        .map(|dir| dir.join(&binary_name))
        .unwrap_or_else(|_| output_dir.join(&binary_name))
}

/// The stamp lives beside the binary as `.<binary_name>.keelstamp`.
fn stamp_path_for(binary_path: &Path) -> Option<PathBuf> {
    let name = binary_path.file_name()?.to_str()?;
    Some(binary_path.with_file_name(format!(".{name}.keelstamp")))
}

fn binary_record(binary: &[u8]) -> String {
    format!("binary {} {:016x}\n", binary.len(), hash_bytes(binary))
}

/// Identify the compiler build itself, so a rebuilt keelc (same version
/// string, different binary — the common dev-loop case) invalidates stamps.
fn compiler_identity() -> Option<String> {
    let exe = env::current_exe().ok()?;
    let meta = fs::metadata(exe).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!(
        "{} {} {mtime}",
        env!("CARGO_PKG_VERSION"),
        meta.len()
    ))
}

/// The Go toolchain is a build input: upgrading Go must invalidate stamps
/// even though the generated Go source is unchanged.
fn go_version() -> Option<String> {
    let output = Command::new("go")
        .arg("version")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8(output.stdout).ok()?;
    let version = version.trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// std's SipHash with fixed keys: deterministic across runs. Not guaranteed
/// stable across Rust releases, which for a cache only costs one spurious
/// rebuild after a toolchain upgrade.
fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(bytes);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp_in(dir: &Path, text: &str, milestone: u32) -> BuildStamp {
        let source = dir.join("app.keel");
        fs::write(&source, text).unwrap();
        BuildStamp::compute(&source, text, milestone).expect("go and current_exe available")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("keelstamp-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_stamp_and_recorded_stamp() {
        let dir = temp_dir("roundtrip");
        let stamp = stamp_in(&dir, "fn main() -> Unit {}\n", 7);
        assert!(!stamp.is_up_to_date(), "no stamp yet");

        fs::write(&stamp.binary_path, b"fake binary").unwrap();
        stamp.record();
        assert!(stamp.is_up_to_date(), "identical inputs and binary");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn source_change_invalidates() {
        let dir = temp_dir("source");
        let stamp = stamp_in(&dir, "fn main() -> Unit {}\n", 7);
        fs::write(&stamp.binary_path, b"fake binary").unwrap();
        stamp.record();

        let changed = stamp_in(&dir, "fn main() -> Unit {}\n// edited\n", 7);
        assert!(!changed.is_up_to_date());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn milestone_change_invalidates() {
        let dir = temp_dir("milestone");
        let stamp = stamp_in(&dir, "fn main() -> Unit {}\n", 7);
        fs::write(&stamp.binary_path, b"fake binary").unwrap();
        stamp.record();

        let other = stamp_in(&dir, "fn main() -> Unit {}\n", 6);
        assert!(!other.is_up_to_date());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_tamper_or_corrupt_stamp_invalidates() {
        let dir = temp_dir("tamper");
        let stamp = stamp_in(&dir, "fn main() -> Unit {}\n", 7);
        fs::write(&stamp.binary_path, b"fake binary").unwrap();
        stamp.record();

        fs::write(&stamp.binary_path, b"tampered").unwrap();
        assert!(!stamp.is_up_to_date(), "binary contents changed");

        fs::write(&stamp.binary_path, b"fake binary").unwrap();
        fs::write(&stamp.stamp_path, "not a stamp").unwrap();
        assert!(!stamp.is_up_to_date(), "corrupt stamp");
        let _ = fs::remove_dir_all(&dir);
    }
}
