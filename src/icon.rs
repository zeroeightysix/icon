use crate::{IconSearch, Theme};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Main struct to locate icon files.
///
/// Create this using [`Icons::new`] for the standard configuration, or use [`IconSearch`] if you
/// wish to tune where icons can be found.
///
/// # Example
///
/// ```rust
/// use icon::Icons;
///
/// Icons::new().find_icon("firefox", 32, 1, "hicolor");
/// ```
pub struct Icons {
    /// Map of "standalone" icons (icons not belonging to any icon theme) to their path.
    pub standalone_icons: HashMap<String, IconFile>,
    /// Map of internal theme names to their corresponding [Theme]
    pub themes: HashMap<OsString, Arc<Theme>>,
}

impl Icons {
    /// Creates a new `Icons`, performing a search in the standard directories.
    ///
    /// This function collects all standalone icons and icon themes on the system.
    /// To configure what directories are searched, use [`IconSearch`] instead.
    pub fn new() -> Self {
        IconSearch::new().search().icons()
    }

    /// Access a known icon theme by name
    pub fn theme(&self, theme_name: &str) -> Option<Arc<Theme>> {
        let theme_name: &OsStr = theme_name.as_ref();
        self.themes.get(theme_name).cloned()
    }

    /// Like [`find_icon`](self.find_icon), with `theme` being `"hicolor"`, which is the default icon theme.
    pub fn find_default_icon(&self, icon_name: &str, size: u32, scale: u32) -> Option<IconFile> {
        self.find_icon(icon_name, size, scale, "hicolor")
    }

    /// Look up an icon by name, size, scale and theme.
    ///
    /// - If no theme by the given name exists, the `"hicolor"` theme (default theme) is used instead.
    /// - If the icon is not found in the provided theme, its parents are checked.
    /// - If the icon is not found in any of the themes, the standalone icon list is checked.
    ///
    /// # Icon matching
    ///
    /// This function will return an icon matching the specified size and scale exactly if it exists.
    /// Otherwise, an icon with the smallest "distance" (in icon size) is returned.
    ///
    /// This will only return `None` if no icon by the specified name exists in the specified theme
    /// and its parents, and no standalone icon by the same name exists either.
    ///
    pub fn find_icon(
        &self,
        icon_name: &str,
        size: u32,
        scale: u32,
        theme: &str,
    ) -> Option<IconFile> {
        if icon_name.is_empty() {
            return None;
        }

        let theme = self.theme(theme).or_else(|| self.theme("hicolor"))?;
        theme
            .find_icon(icon_name, size, scale)
            .or_else(|| self.find_standalone_icon(icon_name))
    }

    /// Look up a standalone icon by name.
    ///
    /// "Standalone" icons are icons that live outside icon themes, residing at the root in the
    /// search directories instead.
    ///
    /// These icons do not have any size or scalability information attached to them.
    pub fn find_standalone_icon(&self, icon_name: &str) -> Option<IconFile> {
        self.standalone_icons.get(icon_name).cloned()
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::new()
    }
}

/// The path to an icon along with its detected file type.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IconFile {
    /// Absolute path to where this icon is found on disk.
    pub path: PathBuf,
    /// The filetype of the icon, derived from its extension. May be `Png`, `Xpm` or `Svg`.
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
    /// `.png` files (Portable Network Graphics)
    Png,
    /// `.xpm` files (X PixMap), an image file format used by the X window system.
    Xpm,
    /// `.svg` files (Scalable Vector Graphics), for images that can be scaled to an arbitrary size.
    Svg,
}

impl FileType {
    /// Get a `FileType` from the file extension of some path.
    pub fn from_path_ext(path: &Path) -> Option<Self> {
        let ext = path.extension()?;
        let ext = ext.to_str()?;

        if ext.eq_ignore_ascii_case("png") {
            Some(FileType::Png)
        } else if ext.eq_ignore_ascii_case("xpm") {
            Some(FileType::Xpm)
        } else if ext.eq_ignore_ascii_case("svg") {
            Some(FileType::Svg)
        } else {
            None
        }
    }

    /// Provides a string representation of this `FileType`.
    ///
    /// Each file type is mapped to its canonical, lowercase file extension ("png", "xpm", "svg").
    pub fn ext(&self) -> &str {
        match self {
            FileType::Png => "png",
            FileType::Xpm => "xpm",
            FileType::Svg => "svg",
        }
    }

    /// Returns an array of all file types that icons may appear as.
    pub const fn types() -> [FileType; 3] {
        [FileType::Png, FileType::Xpm, FileType::Svg]
    }
}

impl Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ext().to_owned())
    }
}
