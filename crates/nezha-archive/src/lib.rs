use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("不支持的压缩格式: {0}")]
    UnsupportedFormat(String),
    #[error("文件未找到: {0}")]
    FileNotFound(String),
    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),
    #[error("ZIP 错误: {0}")]
    Zip(String),
    #[cfg(feature = "sevenz")]
    #[error("7Z 错误: {0}")]
    SevenZ(String),
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    #[error("TAR 错误: {0}")]
    Tar(String),
}

/// 统一封装各种压缩格式的读取。
pub struct Archive {
    inner: ArchiveInner,
}

enum ArchiveInner {
    /// ZIP 支持随机访问，不需要预解压。
    Zip(std::cell::RefCell<zip::ZipArchive<File>>),
    /// 7Z / TAR 系列预先将包内 MIDI 文件解压到内存，后续 O(1) 读取。
    Memory(HashMap<String, Vec<u8>>),
}

impl fmt::Debug for Archive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            ArchiveInner::Zip(_) => f.debug_struct("Archive").field("format", &"zip").finish(),
            ArchiveInner::Memory(m) => f
                .debug_struct("Archive")
                .field("format", &"memory")
                .field("entries", &m.len())
                .finish(),
        }
    }
}

impl Archive {
    /// 根据扩展名自动识别压缩格式并打开。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        let path = path.as_ref();
        let format = detect_format(path);

        let inner = match format {
            Some(Format::Zip) => Self::open_zip(path)?,
            #[cfg(feature = "sevenz")]
            Some(Format::SevenZ) => Self::open_sevenz(path)?,
            #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
            Some(Format::TarGz) | Some(Format::TarXz) | Some(Format::Tar) => {
                Self::open_tar(path, format.unwrap())?
            }
            _ => {
                return Err(ArchiveError::UnsupportedFormat(
                    path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                ));
            }
        };

        Ok(Self { inner })
    }

    /// 列出包内所有 .mid / .midi 文件，按文件名 A-Z 排序。
    pub fn list_midi_files(&self) -> Vec<ArchiveEntry> {
        let mut entries = match &self.inner {
            ArchiveInner::Zip(zip) => {
                let mut list = Vec::new();
                let mut zip = zip.borrow_mut();
                for i in 0..zip.len() {
                    if let Ok(file) = zip.by_index(i) {
                        let name = file.name().to_string();
                        if is_midi_file(&name) {
                            list.push(ArchiveEntry {
                                name,
                                size: file.size(),
                            });
                        }
                    }
                }
                list
            }
            ArchiveInner::Memory(map) => map
                .iter()
                .map(|(name, data)| ArchiveEntry {
                    name: name.clone(),
                    size: data.len() as u64,
                })
                .collect(),
        };

        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        entries
    }

    /// 读取包内指定名称的文件内容。
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        match &self.inner {
            ArchiveInner::Zip(zip) => {
                let mut zip = zip.borrow_mut();
                let mut file = zip
                    .by_name(name)
                    .map_err(|_| ArchiveError::FileNotFound(name.to_string()))?;
                let mut buf = Vec::with_capacity(file.size() as usize);
                file.read_to_end(&mut buf)?;
                Ok(buf)
            }
            ArchiveInner::Memory(map) => map
                .get(name)
                .cloned()
                .ok_or_else(|| ArchiveError::FileNotFound(name.to_string())),
        }
    }

    // ------------------------------------------------------------------
    // ZIP
    // ------------------------------------------------------------------
    fn open_zip(path: &Path) -> Result<ArchiveInner, ArchiveError> {
        let file = File::open(path)?;
        let archive = zip::ZipArchive::new(file).map_err(|e| ArchiveError::Zip(e.to_string()))?;
        Ok(ArchiveInner::Zip(std::cell::RefCell::new(archive)))
    }

    // ------------------------------------------------------------------
    // 7Z
    // ------------------------------------------------------------------
    #[cfg(feature = "sevenz")]
    fn open_sevenz(path: &Path) -> Result<ArchiveInner, ArchiveError> {
        let file = File::open(path)?;
        let size = file.metadata()?.len();
        let mut reader = sevenz_rust::SevenZReader::new(file, size, sevenz_rust::Password::empty())
            .map_err(|e| ArchiveError::SevenZ(format!("{e:?}")))?;

        let mut map = HashMap::new();
        reader
            .for_each_entries(|entry, src| {
                if is_midi_file(&entry.name) {
                    let mut buf = Vec::with_capacity(entry.size() as usize);
                    if src.read_to_end(&mut buf).is_ok() {
                        map.insert(entry.name.clone(), buf);
                    }
                }
                Ok(true)
            })
            .map_err(|e| ArchiveError::SevenZ(format!("{e:?}")))?;

        Ok(ArchiveInner::Memory(map))
    }

    // ------------------------------------------------------------------
    // TAR (plain / gz / xz)
    // ------------------------------------------------------------------
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    fn open_tar(path: &Path, format: Format) -> Result<ArchiveInner, ArchiveError> {
        let mut file = File::open(path)?;
        let mut raw = Vec::new();

        match format {
            #[cfg(feature = "tar-gz")]
            Format::TarGz => {
                let mut decoder = flate2::read::GzDecoder::new(file);
                decoder.read_to_end(&mut raw)?;
            }
            #[cfg(feature = "tar-xz")]
            Format::TarXz => {
                let mut file = std::io::BufReader::new(file);
                lzma_rs::xz_decompress(&mut file, &mut raw)
                    .map_err(|e| ArchiveError::Tar(format!("{e:?}")))?;
            }
            Format::Tar => {
                file.read_to_end(&mut raw)?;
            }
            _ => unreachable!(),
        }

        let mut archive = tar::Archive::new(Cursor::new(&raw));
        let mut map = HashMap::new();

        for entry in archive
            .entries()
            .map_err(|e| ArchiveError::Tar(e.to_string()))?
        {
            let mut entry = entry.map_err(|e| ArchiveError::Tar(e.to_string()))?;
            let name = entry.path().map_err(|e| ArchiveError::Tar(e.to_string()))?;
            let name = name.to_string_lossy().to_string();
            if is_midi_file(&name) {
                let mut buf = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut buf)?;
                map.insert(name, buf);
            }
        }

        Ok(ArchiveInner::Memory(map))
    }
}

// ----------------------------------------------------------------------
// 辅助函数
// ----------------------------------------------------------------------

fn is_midi_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".mid") || lower.ends_with(".midi")
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Zip,
    #[cfg(feature = "sevenz")]
    SevenZ,
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    TarGz,
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    TarXz,
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    Tar,
}

fn detect_format(path: &Path) -> Option<Format> {
    let name = path.file_name()?.to_str()?.to_lowercase();

    if name.ends_with(".zip") {
        return Some(Format::Zip);
    }
    #[cfg(feature = "sevenz")]
    if name.ends_with(".7z") {
        return Some(Format::SevenZ);
    }
    #[cfg(any(feature = "tar-gz", feature = "tar-xz"))]
    {
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Format::TarGz);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Some(Format::TarXz);
        }
        if name.ends_with(".tar") {
            return Some(Format::Tar);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let path = dir.path().join("test.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        zip.start_file("beta.mid", options).unwrap();
        zip.write_all(b"MIDI-beta").unwrap();

        zip.start_file("alpha.mid", options).unwrap();
        zip.write_all(b"MIDI-alpha").unwrap();

        zip.start_file("readme.txt", options).unwrap();
        zip.write_all(b"not a midi").unwrap();

        zip.finish().unwrap();
        path
    }

    #[test]
    fn test_zip_list_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_test_zip(&dir);

        let archive = Archive::open(&path).unwrap();
        let entries = archive.list_midi_files();

        assert_eq!(entries.len(), 2);
        // A-Z 排序：alpha 在 beta 前面
        assert_eq!(entries[0].name, "alpha.mid");
        assert_eq!(entries[0].size, 10);
        assert_eq!(entries[1].name, "beta.mid");
        assert_eq!(entries[1].size, 9);

        let data = archive.read_file("alpha.mid").unwrap();
        assert_eq!(data, b"MIDI-alpha");
    }

    #[test]
    fn test_unsupported_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.unknown");
        File::create(&path).unwrap();

        let err = Archive::open(&path).unwrap_err();
        assert!(matches!(err, ArchiveError::UnsupportedFormat(_)));
    }

    #[test]
    fn test_read_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_test_zip(&dir);

        let archive = Archive::open(&path).unwrap();
        let err = archive.read_file("ghost.mid").unwrap_err();
        assert!(matches!(err, ArchiveError::FileNotFound(_)));
    }
}
