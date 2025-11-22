use crate::{DirectoryIndex, IconSearch, Theme};
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
    /// Map of internal theme names to their corresponding [`Theme`]
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

    /// Find all icons in all themes, in all of their directories.
    ///
    /// Also see [`find_all_icons_filtered`](Icons::find_all_icons_filtered).
    pub fn find_all_icons(&self) -> impl Iterator<Item = (Arc<Theme>, &DirectoryIndex, IconFile)> {
        self.find_all_icons_filtered(|_| true, |_| true, |_| true)
    }

    /// Find all icons in all themes, in all of their directories, filtered at each stage by a predicate.
    ///
    /// This happens lazily: the function returns an iterator that only does the required work
    /// when you advance it.
    ///
    /// <div class="warning">
    ///
    /// The output of this function does **not** include standalone icons.
    /// If you need a full list of icons, use this method and chain it together with the content of
    /// [`standalone_icons`](Icons#structfield.standalone_icons)
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// This method is **not** meant for finding icons by name / size; use [`find_icon`](Icons::find_icon) for that.
    ///
    /// </div>
    ///
    /// # Example
    ///
    /// Find all icons belonging to a theme called "Adwaita", in a directory for icons size ≤ 128px:
    ///
    /// ```rust
    /// use icon::{Icons, IconFile};
    ///
    /// let icons = Icons::new();
    /// let adwaita: Vec<IconFile> = icons
    ///     .find_all_icons_filtered(
    ///         |theme| theme.info.internal_name == "Adwaita",
    ///         |dir| dir.size <= 128,
    ///         |_| true,
    ///     )
    ///     .map(|(_, _, icon)| icon)
    ///     .collect();
    /// ```
    pub fn find_all_icons_filtered<'a>(
        &'a self,
        filter_theme: impl Fn(&Theme) -> bool + 'a,
        filter_directory: impl Fn(&DirectoryIndex) -> bool + 'a,
        filter_icon: impl Fn(&IconFile) -> bool + Clone + 'a,
    ) -> impl Iterator<Item = (Arc<Theme>, &'a DirectoryIndex, IconFile)> {
        // This function conjures up a big iterator over all icons,
        // in all themes, in all theme directories. It does that by chaining (with `zip` and `repeat`)
        // each "level" of the search together:

        // First, find each icon theme, filtered by the `filter_theme` argument:
        let themes = self
            .themes
            .iter()
            .map(|(_, theme)| theme)
            .filter(move |theme| filter_theme(theme.as_ref()));

        // Create an iterator that yields each icon theme × icon theme's directories
        // Item = (&Arc<Theme>, &DirectoryIndex)
        let dirs = themes
            .flat_map(|theme| {
                std::iter::zip(
                    std::iter::repeat(theme),
                    theme.info.index.directories.iter(),
                )
            })
            .filter(move |(_, dir)| filter_directory(dir));

        // Then, for each pair of Theme and DirectoryIndex,
        // find all files in each suitable directory (which may be multiple, if the theme has many
        // base directories).
        // Item = ((&Arc<Theme>, &DirectoryIndex), IconFile)
        let icons = dirs
            .flat_map(move |(theme, dir)| {
                // Each "dir" may map to multiple actual fs directories if the theme
                // has multiple base_dirs.
                let filter_icon = filter_icon.clone();
                let dir_file_iterator = theme
                    .info
                    .base_dirs
                    .iter()
                    .map(|base_dir| base_dir.join(&dir.directory_name))
                    .flat_map(|dir| dir.read_dir()) // Skip directories we can't read.
                    .flatten() // Flatten out the dir iterator,
                    .flatten() // and skip Err entries.
                    .flat_map(|dir_entry| IconFile::from_path_buf(dir_entry.path())) // And then skip all files that aren't icons.
                    .filter(move |icon| filter_icon(icon));

                std::iter::zip(std::iter::repeat((theme, dir)), dir_file_iterator)
            })
            // And finally, turn the nested tuple ((a,b), c) into (a, b, c)
            .map(|((a, b), c)| /*uncurry*/ (a.clone(), b, c));

        icons
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
    path: PathBuf,
    /// The filetype of the icon, derived from its extension. May be `Png`, `Xpm` or `Svg`.
    file_type: FileType,
}

impl IconFile {
    /// Derive the icon name from its path.
    pub fn icon_name(&self) -> &str {
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("protected by type's constructor")
    }

    /// Create an `IconFile` from a filesystem path, deriving its filetype from its extension.
    pub fn from_path(path: &Path) -> Option<IconFile> {
        Self::from_path_buf(path.to_owned())
    }

    /// Create an `IconFile` from an owned filesystem path, deriving its filetype from its extension.
    ///
    /// Returns `None` if the provided path does not have a name or extension valid for icons.
    pub fn from_path_buf(path_buf: PathBuf) -> Option<IconFile> {
        // An icon file must have a file stem.
        path_buf.file_stem()?;

        let file_type = FileType::from_path_ext(&path_buf)?;

        Some(IconFile {
            path: path_buf,
            file_type,
        })
    }

    /// Returns the path associated with this icon
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns this icon's file type
    pub fn file_type(&self) -> FileType {
        self.file_type
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

#[cfg(test)]
mod test {
    use crate::IconFile;
    use crate::search::test::test_search;
    use std::collections::HashMap;

    #[test]
    fn test_find_all_icons() {
        let icons = test_search().search().icons();
        let mut map: HashMap<String, Vec<IconFile>> = Default::default();
        for (_, _, icon) in icons.find_all_icons() {
            map.entry(icon.icon_name().to_owned())
                .or_insert_with(Default::default)
                .push(icon)
        }

        // "beautiful sunset" has 3 icons:
        assert_eq!(map["beautiful sunset"].len(), 3);
        // "happy" has 2:
        assert_eq!(map["happy"].len(), 2);
        // and "pixel" appears once:
        assert_eq!(map["pixel"].len(), 1);

        // "beautiful sunset" has one .xpm file:
        assert_eq!(
            map["beautiful sunset"]
                .iter()
                .filter(|ico| ico.file_type() == crate::FileType::Xpm)
                .count(),
            1
        );
    }
}
