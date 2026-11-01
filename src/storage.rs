//! Filesystem mechanics shared by session and JSON exports.
//!
//! Output directories must be trusted. A rename protects readers from partial
//! output, not from an attacker who can replace entries in the parent directory.
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn private_options() -> OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options.mode(0o600);
        options
    }
    #[cfg(not(unix))]
    OpenOptions::new()
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // Also covers failed serialization, disk-full, and failed rename.
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn atomic_write(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> io::Result<()>,
) -> io::Result<()> {
    if path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output path has no file name",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = None;
    for _ in 0..32 {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let name = parent.join(format!(".gdl90-{}-{sequence}.tmp", std::process::id()));
        match private_options().write(true).create_new(true).open(&name) {
            Ok(file) => {
                temporary = Some((TemporaryFile(name), file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    let (temporary, file) = temporary.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary output names are occupied",
        )
    })?;
    let mut writer = BufWriter::new(file);
    write(&mut writer)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    fs::rename(&temporary.0, path)?;
    // Failure here means the replacement is visible but directory durability
    // could not be confirmed. Do not remove the successfully published output.
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_write_preserves_existing_output_and_removes_temporary() {
        let dir = std::env::temp_dir().join(format!("gdl90-atomic-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.txt");
        fs::write(&path, b"original").unwrap();
        let result = atomic_write(&path, |writer| {
            writer.write_all(b"incomplete")?;
            Err(io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        atomic_write(&path, |writer| writer.write_all(b"replacement")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
