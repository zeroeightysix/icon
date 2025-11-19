use std::fmt::Display;
use std::path::{Path, PathBuf};

/// The path to an icon along with its detected file type.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IconFile {
    /// Absolute path to where this icon is found on disk.
    pub path: PathBuf,
    /// The filetype of the icon, derived from its extension. May be `Png`, `Xmp` or `Svg`.
    pub file_type: FileType,
}

impl IconFile {
    /// Create an `IconFile` from a filesystem path, deriving its filetype from its extension.
    pub fn from_path(path: &Path) -> Option<IconFile> {
        let file_type = FileType::from_path_ext(path)?;

        Some(IconFile {
            path: path.to_owned(),
            file_type,
        })
    }
}

/// Supported image file formats for icons.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FileType {
    Png,
    Xmp,
    Svg,
}

impl FileType {
    /// Get a `FileType` from the file extension of some path.
    pub fn from_path_ext(path: &Path) -> Option<Self> {
        let ext = path.extension()?;
        let ext = ext.to_str()?;

        if ext.eq_ignore_ascii_case("png") {
            Some(FileType::Png)
        } else if ext.eq_ignore_ascii_case("xmp") {
            Some(FileType::Xmp)
        } else if ext.eq_ignore_ascii_case("svg") {
            Some(FileType::Svg)
        } else {
            None
        }
    }

    /// Provides a string representation of this `FileType`.
    ///
    /// Each file type is mapped to its canonical, lowercase file extension ("png", "xmp", "svg").
    pub fn ext(&self) -> &str {
        match self {
            FileType::Png => "png",
            FileType::Xmp => "xmp",
            FileType::Svg => "svg",
        }
    }

    /// Returns an array of all file types that icons may appear as.
    pub const fn types() -> [FileType; 3] {
        [FileType::Png, FileType::Xmp, FileType::Svg]
    }
}

impl Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ext().to_owned())
    }
}
