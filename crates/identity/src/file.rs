//! Encrypted key files on desktop, as identity/file.go and public_file.go.
//! Both the active file and exported files remain encrypted.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use crate::{Error, backup::MAX_BACKUP_SIZE, key::PUBLIC_KEY_SIZE};

pub fn read_backup(path: &Path) -> Result<Vec<u8>, Error> {
    let mut f = fs::File::open(path)?;
    let mut b = Vec::new();
    (&mut f)
        .take(MAX_BACKUP_SIZE as u64 + 1)
        .read_to_end(&mut b)?;
    if b.len() > MAX_BACKUP_SIZE {
        return Err(Error::new("identity: backup is too large"));
    }
    Ok(b)
}

/// Never overwrites an existing key or backup. An interrupted write is removed.
pub fn write_backup(path: &Path, data: &[u8]) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        create_private_dir(dir)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    let written = f
        .write_all(data)
        .and_then(|_| f.sync_all())
        .map_err(Error::from);
    drop(f);
    if written.is_err() {
        let _ = fs::remove_file(path);
    }
    written
}

/// Stores printable Ed25519 public bytes atomically. It is a convenience
/// file; authorization derives the key from the unlocked seed.
pub fn write_public_key(path: &Path, public: &[u8]) -> Result<(), Error> {
    if public.len() != PUBLIC_KEY_SIZE {
        return Err(Error::new("identity: invalid public key"));
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    create_private_dir(dir)?;
    let text = format!("{}\n", hex::encode(public));
    write_atomically(dir, ".public-key-", path, text.as_bytes(), 0o644)
}

/// Replaces `path` through a temporary file in the same directory, as Go's
/// CreateTemp/Rename pattern. A failure leaves `path` untouched.
pub fn write_atomically(
    dir: &Path,
    prefix: &str,
    path: &Path,
    data: &[u8],
    mode: u32,
) -> Result<(), Error> {
    let _ = mode;
    let mut random = [0u8; 8];
    getrandom::getrandom(&mut random)
        .map_err(|_| Error::new("identity: no secure randomness available"))?;
    let temp = dir.join(format!("{prefix}{}", hex::encode(random)));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    let mut f = options.open(&temp)?;
    let result = f
        .write_all(data)
        .and_then(|_| f.sync_all())
        .map_err(Error::from)
        .and_then(|_| {
            drop(f);
            fs::rename(&temp, path).map_err(Error::from)
        });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn create_private_dir(dir: &Path) -> Result<(), Error> {
    if dir.as_os_str().is_empty() || dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(dir)?;
    Ok(())
}
