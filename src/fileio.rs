//! Сохранение через временный файл в том же каталоге: исходный файл
//! заменяется только после успешной записи и синхронизации нового.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    // Сохраняем поведение записи через символическую ссылку.
    let target = if path.is_symlink() { fs::canonicalize(path)? } else { path.to_path_buf() };
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "нет имени файла"))?;
    let (temp, mut file) = loop {
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(".{}.{}.tmp", std::process::id(), NEXT_FILE.fetch_add(1, Ordering::Relaxed)));
        let temp = parent.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (TempFile(temp), file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    if let Ok(metadata) = fs::metadata(&target) {
        if metadata.permissions().readonly() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "файл доступен только для чтения"));
        }
        file.set_permissions(metadata.permissions())?;
    }
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp.0, target)
}

#[cfg(test)]
mod tests;
