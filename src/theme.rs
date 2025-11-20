use crate::ThemeParseError::MissingRequiredAttribute;
use crate::icon::IconFile;
use freedesktop_entry_parser::low_level::{SectionBytes, SectionBytesIter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// An icon theme.
pub struct Theme {
    /// Properties of this theme and all of its subdirectories.
    pub info: ThemeInfo,
    /// References to the `Theme`s that this theme depends on.
    ///
    /// When querying for an icon that doesn't exist in this theme, the themes in its `inherits_from`
    /// list will be checked for that icon instead.
    pub inherits_from: Vec<Arc<Theme>>,
}

impl Theme {
    /// Find an icon in this theme or any of its dependencies, with scale equal to 1.
    ///
    /// Also see [find_icon](Theme::find_icon)
    pub fn find_icon_unscaled(&self, icon_name: &str, size: u32) -> Option<IconFile> {
        self.find_icon(icon_name, size, 1)
    }

    /// Find an icon in this theme or any of its dependencies.
    ///
    /// Arguments:
    /// - `icon_name`: the canonical name of the icon **without** file extension.
    /// - `size`: the size, in pixels, desired. The returned icon may not be this exact size in case an exact match couldn't be found.
    /// - `scale`: the scale at which the icon will be displayed.
    pub fn find_icon(&self, icon_name: &str, size: u32, scale: u32) -> Option<IconFile> {
        self.find_icon_here(icon_name, size, scale).or_else(|| {
            // or find it in one of our parents
            self.inherits_from
                .iter()
                .find_map(|theme| theme.find_icon_here(icon_name, size, scale))
        })
    }

    /// Find an icon in this theme only.
    ///
    /// Do not use this function if you need normal icon finding behaviour: use [find_icon](Theme::find_icon) instead.
    pub fn find_icon_here(&self, icon_name: &str, size: u32, scale: u32) -> Option<IconFile> {
        const EXTENSIONS: [&str; 3] = ["png", "xpm", "svg"];
        let file_names = EXTENSIONS.map(|ext| format!("{icon_name}.{ext}"));

        let base_dirs = &self.info.base_dirs;

        let sub_dirs = &self.info.index.directories;
        // first, try to find an exact icon size match:
        let exact_sub_dirs = sub_dirs
            .iter()
            .filter(|sub_dir| sub_dir.matches_size(size, scale));

        for base_dir in base_dirs {
            for sub_dir in exact_sub_dirs.clone() {
                for file_name in &file_names {
                    let path = base_dir
                        .join(sub_dir.directory_name.as_str())
                        .join(file_name);

                    if path.exists()
                        && let Some(file) = IconFile::from_path(&path)
                    {
                        // exact match!
                        return Some(file);
                    }
                }
            }
        }

        drop(exact_sub_dirs);

        // no exact match: try to find a match as close as possible instead.
        let mut min_dist = u32::MAX;
        let mut best_icon = None;

        for base_dir in base_dirs {
            for sub_dir in sub_dirs {
                let distance = sub_dir.size_distance(size, scale);

                if distance < min_dist {
                    for file_name in &file_names {
                        let path = base_dir
                            .join(sub_dir.directory_name.as_str())
                            .join(file_name);
                        if path.exists()
                            && let Some(file) = IconFile::from_path(&path)
                        {
                            min_dist = distance;
                            best_icon = Some(file);
                        }
                    }
                }
            }
        }

        best_icon
    }
}

/// Information about an icon theme.
///
/// Its formal description (called the index) can be found in the `index` field.
pub struct ThemeInfo {
    /// The name of the directory wherein this theme lives.
    ///
    /// This is different from the theme's actual name, which is specified in its index. (See `index.name`)
    pub internal_name: String,
    /// The directories in which this theme's icons live.
    ///
    /// The Icon Theme specification allows a theme to be split up over multiple directories
    /// (of the same internal name) in each of the base directories applications look for themes.
    /// This list holds the paths to all directories where this theme is specified.
    pub base_dirs: Vec<PathBuf>,
    /// Although icon themes may be split up over multiple directories, each icon theme is only
    /// allowed one `index.theme` file to dictate the theme's properties. Applications must use the
    /// first `index.theme` file they find when searching base directories; this field holds the
    /// path to that file.
    pub index_location: PathBuf,
    /// The contents of the `index.theme` file.
    pub index: ThemeIndex,
    // additional groups?
}

/// An error occurred during theme index parsing.
///
/// This type is returned by [ThemeIndex::parse] and indirectly by [ThemeInfo::new_from_folders].
#[derive(Debug, thiserror::Error)]
pub enum ThemeParseError {
    /// Missing the "Icon Theme" section.
    #[error("missing Icon Theme index or section")]
    NotAnIconTheme,
    /// An attribute that is required, is missing.
    #[error("missing attribute `{0}`")]
    MissingRequiredAttribute(&'static str),
    /// The file isn't encoded in UTF-8.
    #[error("the input wasn't in utf-8")]
    NotUtf8(#[from] std::str::Utf8Error),
    /// Couldn't parse a `bool`ean where one was expected.
    #[error("a bool was expected but failed to parse")]
    ParseBoolError(#[from] std::str::ParseBoolError),
    /// Couldn't parse a `bool`ean where one was expected.
    #[error("a number was expected but failed to parse")]
    ParseNumError(#[from] std::num::ParseIntError),
    /// Couldn't parse a [DirectoryType](DirectoryType) where one was expected.
    #[error("A directory type was invalid")]
    InvalidDirectoryType,
    /// The file was not properly formatted as a freedesktop entry file.
    ///
    /// Entry files look like `.ini` files, but they are not the same.
    /// Check out the specification for entry files [here](https://specifications.freedesktop.org/desktop-entry/latest/basic-format.html).
    #[error("invalid format for a freedesktop entry file")]
    ParseError(#[from] freedesktop_entry_parser::low_level::ParseError),
}

impl ThemeInfo {
    /// Create a new `ThemeInfo` from a theme's internal name and the folders at which the theme
    /// lives.
    ///
    /// This function will parse the first `index.theme` file found in the directories passed in.
    pub fn new_from_folders(internal_name: String, folders: Vec<PathBuf>) -> std::io::Result<Self> {
        let index_location = folders
            .iter()
            .map(|f| f.join("index.theme"))
            .find(|index_path| index_path.exists())
            .ok_or_else(|| std::io::Error::other(ThemeParseError::NotAnIconTheme))?;

        let index = ThemeIndex::parse_from_file(index_location.as_path())?;

        Ok(Self {
            internal_name,
            base_dirs: folders,
            index_location,
            index,
        })
    }
}

/// The "formal description" of a theme as specified by the Icon Theme specification.
///
/// Every icon theme must 'describe' itself using an index file. It contains useful information such
/// as a human-readable name for the theme, which themes it depends on (i.e. fallbacks),
/// and a complete listing of all directories associated with the icon theme along with their
/// properties.
///
/// All doc comments in *italics* below are copy-pasted from the XDG Icon Theme Specification.
pub struct ThemeIndex {
    /// *Short name of the icon theme, used in e.g. lists when selecting themes.*
    pub name: String,
    /// *Longer string describing the theme*
    pub comment: String,
    /// *The name of the theme that this theme inherits from. If an icon name is not found in the current theme, it is searched for in the inherited theme (and recursively in all the inherited themes).*
    ///
    /// *If no theme is specified, implementations are required to add the "hicolor" theme to the inheritance tree. An implementation may optionally add other default themes in between the last specified theme and the hicolor theme.*
    ///
    /// *Themes that are inherited from explicitly must be present on the system.*
    pub inherits: Vec<String>,
    /// Directories associated with this icon theme. This compounds the "Directories" **and**
    /// "ScaledDirectories" entries of the index.
    ///
    /// "Directories": *List of subdirectories for this theme. For every subdirectory there must be a section in the `index.theme` file describing that directory.* \
    /// "ScaledDirectories": *Additional list of subdirectories for this theme, in addition to the ones in Directories. These directories should only be read by implementations supporting scaled directories and was added to keep compatibility with old implementations that don't support these.*
    pub directories: Vec<DirectoryIndex>,
    /// *Whether to hide the theme in a theme selection user interface. This is used for things such as fallback-themes that are not supposed to be visible to the user.*
    pub hidden: bool,
    /// *The name of an icon that should be used as an example of how this theme looks.*
    pub example: Option<String>,
}

impl ThemeIndex {
    /// Parse an icon theme index file by path.
    ///
    /// If this function fails to read the file, it will return the IO error that caused failure. \
    /// If parsing the contents of the file failed, it will return [std::io::Error::other] with the
    ///   responsible [ThemeParseError] inside.
    pub fn parse_from_file(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let index = ThemeIndex::parse(&bytes).map_err(std::io::Error::other)?;

        Ok(index)
    }

    /// Parse an icon theme index directory from the content, in bytes, of the file.
    ///
    /// See [ThemeParseError] for the errors this function may return.
    pub fn parse(bytes: &[u8]) -> Result<Self, ThemeParseError> {
        let mut entry: SectionBytesIter = freedesktop_entry_parser::low_level::parse_entry(bytes);

        let icon_theme_section: SectionBytes =
            entry.next().ok_or(ThemeParseError::NotAnIconTheme)??;
        let name: &str = find_attr_req(&icon_theme_section, "Name")?;

        // SPEC: `Comment` is required, but most icon theme developers can't be arsed to
        // include it! To make `icon` practical, we choose a default of an empty string instead.
        // `let comment = find_attr_req(&icon_theme_section, "Comment")?;`
        let comment = find_attr(&icon_theme_section, "Comment")?.unwrap_or("");
        // If no theme is specified, implementations are required to add the "hicolor" theme to the inheritance tree.
        let inherits = find_attr(&icon_theme_section, "Inherits")?
            .iter()
            .flat_map(|s| s.split(',')) // `inherits` is a comma-separated string list
            .map(Into::into)
            .collect::<Vec<_>>();
        let directories = find_attr_req(&icon_theme_section, "Directories")?
            .split(',')
            .collect::<Vec<_>>();
        let scaled_directories = find_attr(&icon_theme_section, "ScaledDirectories")?
            .map(|s| s.split(',').collect::<Vec<_>>());
        let hidden = find_attr(&icon_theme_section, "Hidden")?
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(false);
        let example = find_attr(&icon_theme_section, "Example")?;

        // all other sections should describe a directory in the directory list
        let directories = entry
            .filter_map(Result::ok)
            .filter_map(|section| {
                let title = str::from_utf8(section.title).ok()?;

                let is_scaled_dir = scaled_directories
                    .as_ref()
                    .map(|d| d.contains(&title))
                    .unwrap_or(false);

                if !directories.contains(&title) && !is_scaled_dir {
                    // this section isn't a listed directory! ignore!
                    return None;
                }

                let mut index = DirectoryIndex::parse(section);

                if is_scaled_dir && let Ok(index) = &mut index {
                    index.is_scaled_dir = true;
                }

                Some(index)
            })
            .collect::<Result<Vec<_>, ThemeParseError>>()?;

        Ok(Self {
            name: name.into(),
            comment: comment.into(),
            inherits,
            directories,
            hidden,
            example: example.map(Into::into),
        })
    }
}

/// The "formal description" of a subdirectory in an Icon Theme, as specified by the Icon Theme
/// specification.
///
/// All doc comments in *italics* below are copy-pasted from the XDG Icon Theme Specification.
pub struct DirectoryIndex {
    /// The name of the subdirectory as found in the theme's index file.
    ///
    /// It is not guaranteed that a subdirectory with the same name actually exists.
    pub directory_name: String,
    /// Is this directory listed as a "normal" subdirectory (holding specific sizes of icons) or a "scaled" directory (holding scalable graphics)?
    pub is_scaled_dir: bool,
    /// *Nominal (unscaled) size of the icons in this directory.*
    ///
    /// This is the only required field; all others assume their default value if not present.
    pub size: u32,
    /// *Target scale of the icons in this directory. Defaults to the value 1 if not present. Any directory with a scale other than 1 should be listed in the ScaledDirectories list rather than Directories for backwards compatibility.*
    pub scale: u32,
    /// *The context the icon is normally used in. This is in detail discussed in [Section 4.1, “Context”](https://specifications.freedesktop.org/icon-theme/latest/#context).*
    pub context: Option<String>,
    /// *The type of icon sizes for the icons in this directory. Valid types are `Fixed`, `Scalable` and `Threshold`. The type decides what other keys in the section are used. If not specified, the default is `Threshold`.*
    pub directory_type: DirectoryType,
    /// *Specifies the maximum (unscaled) size that the icons in this directory can be scaled to. Defaults to the value of `size` if not present.*
    pub max_size: u32,
    /// *Specifies the minimum (unscaled) size that the icons in this directory can be scaled to. Defaults to the value of `size` if not present.*
    pub min_size: u32,
    /// *The icons in this directory can be used if the size differ at most this much from the desired (unscaled) size. Defaults to *2* if not present.*
    pub threshold: u32,
    // pub additional_values: HashMap<String, String>,
}

impl DirectoryIndex {
    fn parse(section: SectionBytes) -> Result<Self, ThemeParseError> {
        let dir_name = str::from_utf8(section.title)?;
        let size: u32 = find_attr_req(&section, "Size")?.parse()?;
        let scale: u32 = find_attr(&section, "Scale")?
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(1);
        let context = find_attr(&section, "Context")?;
        // Valid types are Fixed, Scalable and Threshold.
        // The type decides what other keys in the section are used.
        // If not specified, the default is Threshold.
        let directory_type = find_attr(&section, "Type")?
            .map(|s| s.try_into())
            .transpose()
            .map_err(|_| ThemeParseError::InvalidDirectoryType)?
            .unwrap_or(DirectoryType::Threshold);
        let max_size = find_attr(&section, "MaxSize")?
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(size);
        let min_size = find_attr(&section, "MinSize")?
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(size);
        let threshold = find_attr(&section, "Threshold")?
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(2);

        Ok(Self {
            directory_name: dir_name.into(),
            is_scaled_dir: scale != 1,
            size,
            scale,
            context: context.map(Into::into),
            directory_type,
            max_size,
            min_size,
            threshold,
        })
    }

    fn size_distance(&self, icon_size: u32, icon_scale: u32) -> u32 {
        let size = icon_size * icon_scale;

        match self.directory_type {
            DirectoryType::Fixed | DirectoryType::Scalable => {
                (self.size * self.scale).abs_diff(size)
            }
            DirectoryType::Threshold => {
                let lower = (self.size - self.threshold) * self.scale;
                let higher = (self.size + self.threshold) * self.scale;

                if size < lower {
                    size.abs_diff(self.min_size * self.scale)
                } else if size > higher {
                    size.abs_diff(self.max_size * self.scale)
                } else {
                    0 // within range -> no distance!
                }
            }
        }
    }

    /// Computes whether this directory "supports" icons with the provided `icon_size` (in pixels)
    /// and scale (as a multiple of the size).
    ///
    /// The behaviour of this method depends on the [DirectoryType](DirectoryIndex#structfield.directory_type) of this directory.
    /// If the type is:
    /// - [DirectoryType::Fixed]: Only icons with the same size and scale as the directory match.
    /// - [DirectoryType::Scalable]: Icons with a size between the directory's `min_size` and `max_size`, and equal scale, match.
    /// - [DirectoryType::Threshold]: `icon_size` may only differ by the amount of `threshold` specified by the directory, and scale must match exactly.
    ///
    /// When this method returns `true`, the "size distance" of the provided size and scale to the directory's size and scale is considered to be 0.
    pub fn matches_size(&self, icon_size: u32, icon_scale: u32) -> bool {
        if self.scale != icon_scale {
            return false;
        }

        match self.directory_type {
            DirectoryType::Fixed => self.size == icon_size,
            DirectoryType::Scalable => {
                let DirectoryIndex {
                    min_size, max_size, ..
                } = *self;

                (min_size..=max_size).contains(&icon_size)
            }
            DirectoryType::Threshold => {
                let DirectoryIndex {
                    threshold, size, ..
                } = *self;

                // The icons in this directory can be used if the size differ at most this much from the desired (unscaled) size
                size.abs_diff(icon_size) <= threshold
            }
        }
    }
}

/// The type of image scaling used for an icon theme subdirectory.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DirectoryType {
    /// Fixed-size images ([FileType::Png](crate::FileType::Png) and [FileType::Xpm](crate::FileType::Xpm)); these icons may not be scaled to any other size.
    Fixed,
    /// For scalable (vector) graphics ([FileType::Svg](crate::FileType::Svg))
    Scalable,
    /// For fixed-size images that may be scaled within a specified threshold.
    ///
    /// This is the default type, and by default the threshold is 2 pixels.
    Threshold,
}

/// The `Default` implementation for `DirectoryType` returns [DirectoryType::Threshold].
///
/// This is because the XDG Icon Theme specification mandates that if the type for a directory is
/// not specified, it is chosen to be `Threshold`.
impl Default for DirectoryType {
    fn default() -> Self {
        DirectoryType::Threshold
    }
}

impl TryFrom<&str> for DirectoryType {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = match value {
            "Fixed" => DirectoryType::Fixed,
            "Scalable" => DirectoryType::Scalable,
            "Threshold" => DirectoryType::Threshold,
            _ => return Err(()),
        };

        Ok(value)
    }
}

fn find_attr<'a>(
    section: &'a SectionBytes,
    name: &str,
) -> Result<Option<&'a str>, std::str::Utf8Error> {
    section
        .attrs
        .iter()
        .find(|attr| attr.name == name.as_bytes() && attr.param.is_none())
        .map(|attr| str::from_utf8(&attr.value))
        .transpose()
}

fn find_attr_req<'a>(
    section: &'a SectionBytes,
    name: &'static str,
) -> Result<&'a str, ThemeParseError> {
    find_attr(section, name)?.ok_or(MissingRequiredAttribute(name))
}

#[cfg(test)]
mod test {
    use crate::Icons;
    use crate::icon::FileType;
    use crate::{DirectoryType, ThemeIndex};
    use std::error::Error;
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn test_find_firefox() {
        let icons = Icons::new();

        let ico = icons.find_default_icon("firefox", 128, 1).unwrap();

        assert_eq!(
            ico.path(),
            Path::new("/usr/share/icons/hicolor/128x128/apps/firefox.png")
        );
        assert_eq!(ico.file_type(), FileType::Png);

        // we should be able to find an icon for a bunch of different sizes
        for size in (16u32..=64).step_by(8) {
            assert!(icons.find_default_icon("firefox", size, 1).is_some());
        }

        assert!(icons.find_default_icon("firefox", 64, 2).is_some());
    }

    #[test]
    fn find_all_desktop_entry_icons() {
        let icons = Icons::new();

        // some desktop files are just packaged poorly.
        // if a test fails here, and you are certain that the icon just straight up doesn't exist,
        // or is in an unfindable place by normal means,
        // disallow it in this list.
        static DISALLOW_LIST: &[&str] = &[
            "imv-dir",
            "imv",
            "io.elementary.granite.demo",
            "java-java-openjdk",
            "jconsole-java-openjdk",
            "jshell-java-openjdk",
            "lstopo",
            "signon-ui",
        ];

        let mut time_taken = Duration::ZERO;
        let mut n = 0;

        for entry in
            freedesktop_desktop_entry::Iter::new(freedesktop_desktop_entry::default_paths())
                .entries(None::<&[&str]>)
        {
            let Some(icon_name) = entry.icon() else {
                continue;
            };

            if Path::new(icon_name).exists() {
                continue; // absolute URLs to icons are OK
            }

            if DISALLOW_LIST
                .iter()
                .any(|x| Some(x.as_ref()) == entry.path.file_stem())
            {
                continue;
            }

            let then = Instant::now();

            // TODO: perhaps our system should expose a way to construct a "composed theme" filter,
            // for cases where you want to search a multitude (or all) themes
            let icon = icons
                .find_icon(icon_name, 32, 1, "gnome")
                .or_else(|| icons.find_icon(icon_name, 32, 1, "breeze"));

            time_taken += Instant::now() - then;
            n += 1;

            assert!(
                icon.is_some(),
                "Icon {icon_name} from desktop entry {:?} missing!!",
                entry.path
            )
        }

        println!("avg {:?} per icon", time_taken / n);
    }

    #[test]
    fn test_parse_example_theme() -> Result<(), Box<dyn Error>> {
        static EXAMPLE: &'static str = include_str!("../resources/example.index.theme");

        let index = ThemeIndex::parse(EXAMPLE.as_bytes())?;

        assert_eq!(index.name, "Birch");
        assert_eq!(index.comment, "Icon theme with a wooden look");
        assert_eq!(index.inherits, vec!["wood", "default"]);

        let directories = index.directories;

        assert_eq!(directories.len(), 7);

        let first_dir_index = &directories[0];
        assert_eq!(first_dir_index.directory_name, "scalable/apps");
        assert_eq!(first_dir_index.is_scaled_dir, false);
        assert_eq!(first_dir_index.size, 48);
        assert_eq!(first_dir_index.scale, 1);
        assert_eq!(first_dir_index.context.as_deref(), Some("Applications"));
        assert_eq!(first_dir_index.directory_type, DirectoryType::Scalable);
        assert_eq!(first_dir_index.max_size, 256);
        assert_eq!(first_dir_index.min_size, 1);
        assert_eq!(first_dir_index.threshold, 2);

        assert_eq!(index.hidden, false);
        assert_eq!(index.example, None);

        Ok(())
    }
}
