use crate::icon::IconFile;
use crate::{Icons, Theme, ThemeInfo, ThemeParseError};
use states::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

macro_rules! states {
    ($($(#[$($attr:tt)*])* $id:ident),*) => {
        mod sealed {
            pub trait Sealed {}
        }

        /// Sealed trait to protect [IconSearch](crate::IconSearch)'s states from external implementations.
        ///
        /// You do not need to know this exists in order to use `icon`.
        pub trait TypeStateProtector: sealed::Sealed {}

        $(
            $(#[$($attr)*])*
            pub struct $id;

            impl sealed::Sealed for $id {}
            impl TypeStateProtector for $id {}
        )*
    };
}

/// Marker types used for the [IconSearch] builder.
///
/// Typically, you won't have to interact with these.
pub mod states {
    states!(
        /// Initial state.
        ///
        /// Configure directories where icons and icon themes may be found.
        ///
        /// Then, proceed to [LocationsFound].
        Initial,
        /// Second state, proceeding [Initial].
        ///
        /// We've found standalone icons and have candidates for where icon themes may live.
        /// If you are only interested in standalone icons or just need a list of icon theme names
        /// (although, watch out: they're _candidates_ and might not be valid icon themes),
        /// you can drop out at this stage.
        LocationsFound,
        /// Third state, proceeding [LocationsFound].
        ///
        /// We've found standalone icons and have parsed all icon themes plus calculated their
        /// inheritance tree. At this stage, you can inspect the results from the search process
        /// and then proceed to the usable icon-finder by calling `collect()`.
        Finished
    );
}

/// Icons and icon themes are looked for in a set of directories.
///
/// By default, that is `$HOME/.icons`, `$XDG_DATA_HOME/icons`, `$XDG_DATA_DIRS/icons` and `/usr/share/pixmaps`.
/// Applications may further add their own icon directories to this list, and users may extend or change the list.
/// The default list may be obtained using the `Default` implementation on `IconSearch` or its `default` method.
///
/// To add directories to the instance, use [`IconSearch::add_directories`].
///
/// To construct a new `IconSearch` from a list, use the `From` implementation or [`IconSearch::new_from`].
///
/// # Example
///
/// ```
/// use icon::IconSearch;
///
/// let icons = IconSearch::new()
///     // (optional) add directories to search
///     .add_directories(["/some/additional/directory/"])
///     // find icons and folders
///     .search()
///     // resolve all icon themes and return an Icons struct which you can use for icon finding!
///     .icons();
/// ```
// #[derive(Debug, Clone)]
pub struct IconSearch<State = Initial> {
    /// The list of directories to search for standalone icons and icon themes
    pub dirs: Vec<PathBuf>,
    icon_locations: Option<IconLocations>,
    icons: Option<Icons>,
    // in fn() so that the compiler doesn't see State as part of this struct,
    // which avoids noise in rustdoc.
    _state: PhantomData<fn() -> State>,
}

impl IconSearch<Initial> {
    // -- STAGE 1: Establish directories wherein to find icons

    /// Constructs a new `IconSearch` from the default directories, which are
    /// - `$HOME/.icons`
    /// - `$XDG_DATA_DIRS/icons`
    /// - `/usr/share/pixmaps`
    ///
    /// If you wish to add directories to those, use this function and then [`add_directories`](Self::add_directories).
    pub fn new() -> Self {
        <Self as Default>::default()
    }

    /// Constructs a new `IconSearch` without any directories to search.
    pub const fn new_empty() -> Self {
        Self::new_from(Vec::new())
    }

    /// Constructs a new `IconSearch` from a list of directories to search.
    pub const fn new_from(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            icon_locations: None,
            icons: None,
            _state: PhantomData,
        }
    }

    /// Adds a list of directories to this `IconSearch`.
    ///
    /// # Example
    ///
    /// ```
    /// use icon::IconSearch;
    ///
    /// let dirs = IconSearch::new()
    ///     .add_directories(["/home/root/.icons"])
    ///     .search()
    ///     .icons();
    /// ```
    pub fn add_directories<I, P>(mut self, directories: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut extra_dirs = directories.into_iter().map(Into::into).collect();
        self.dirs.append(&mut extra_dirs);

        self
    }

    // -- STAGE 2: In search dirs, find standalone icons and directories that may be icon themes

    fn find_icon_locations(&self) -> IconLocations {
        // "Each theme is stored as subdirectories of the base directories"

        let (dirs, files) = self
            .dirs
            .iter()
            .flat_map(|base_dir| base_dir.read_dir()) // read the entries in each base dir
            .flatten() // merge all the iterators
            .flatten() // remove Err entries
            .filter_map(|entry| Some((entry.file_type().ok()?, entry))) // get file type for each entry and skip if fail
            .partition::<Vec<_>, _>(|(ft, entry)| {
                ft.is_dir() || (entry.path().extension().is_none() && ft.is_symlink())
            });

        // icons at the top-level in a base_dir don't belong to a theme, but must still be able to be found!
        let files = files
            .into_iter()
            .flat_map(|(_, entry)| IconFile::from_path(&entry.path()))
            .collect::<Vec<_>>();

        // "In at least one of the theme directories there must be a file called
        // index.theme that describes the theme. The first index.theme found while
        // searching the base directories in order is used"

        // For each theme name, create a list of directories where it may be found:
        let mut themes_directories: HashMap<OsString, Vec<PathBuf>> = HashMap::new();
        for (_, dir) in dirs {
            let theme_name = dir.file_name();

            themes_directories
                .entry(theme_name)
                .or_default()
                .push(dir.path());
        }

        IconLocations {
            standalone_icons: files,
            themes_directories,
            #[cfg(feature = "full-search")]
            full_icon_map: None,
        }
    }

    /// Find icons and icon themes in the configured search directories.
    ///
    /// This function proceeds the [`IconSearch`] to the [next stage](LocationsFound).
    pub fn search(self) -> IconSearch<LocationsFound> {
        let icon_locations = self.find_icon_locations();

        IconSearch::<LocationsFound> {
            dirs: self.dirs,
            icon_locations: Some(icon_locations),
            icons: None,
            _state: PhantomData,
        }
    }
}

impl IconSearch<LocationsFound> {
    /// Borrows the [`IconLocations`] from this `IconSearch` for inspection.
    pub fn icon_locations(&self) -> &IconLocations {
        self.icon_locations
            .as_ref()
            .expect("guaranteed by type-state")
    }

    /// Consume this `IconSearch` to expose its [`IconLocations`].
    ///
    /// Contained search directories are lost.
    pub fn into_icon_locations(self) -> IconLocations {
        self.icon_locations.expect("guaranteed by type-state")
    }

    // -- STAGE 3: We have icon theme candidates, so it's time to resolve them.

    fn finish(self) -> IconSearch<Finished> {
        let icons = self.icon_locations.expect("guaranteed by type-state");
        let icons = icons.icons();

        IconSearch {
            dirs: self.dirs,
            icon_locations: None, // consumed!
            icons: Some(icons),
            _state: PhantomData,
        }
    }

    /// Finish icon finding by parsing, validating, and resolving (parents of) all icon themes
    /// found.
    pub fn icons(self) -> Icons {
        self.finish().icons()
    }
}

impl IconSearch<Finished> {
    /// Consume this `IconSearch` to expose its [`Icons`].
    ///
    /// Contained search directories are lost.
    pub fn icons(self) -> Icons {
        self.icons.expect("guaranteed by type-state")
    }
}

/// Locations icons may be found at.
///
/// This consists of two parts:
/// - A list of standalone icons. These are "loose" icons found in the searched base directories. They do not belong to any theme.
/// - A map of icon theme identifiers ("internal name"s) to all directories where that icon theme's icons live.
///   This is a list because icon themes may be split up over multiple base directories.
#[derive(Debug)]
pub struct IconLocations {
    /// List of icons not belonging to any theme.
    pub standalone_icons: Vec<IconFile>,
    /// Map of icon theme identifiers to the directories where the icons live.
    pub themes_directories: HashMap<OsString, Vec<PathBuf>>,
    #[cfg(feature = "full-search")]
    full_icon_map: Option<HashMap<String, Vec<IconFile>>>,
}

impl IconLocations {
    /// Find icon locations from a given `IconSearch` (in initial state).
    ///
    /// There are few reasons to use this function.
    /// Prefer following the normal flow instead:
    /// ```rust
    /// use icon::IconSearch;
    /// let search = IconSearch::new()
    ///     .search();
    ///
    /// let locations = search.into_icon_locations();
    /// ```
    pub fn from_icon_search(dirs: &IconSearch<Initial>) -> Self {
        dirs.find_icon_locations()
    }

    /// Search for all icons in all themes and store the result.
    #[cfg(feature = "full-search")]
    pub fn full_icon_search(&mut self) -> &HashMap<String, Vec<IconFile>> {
        // First, initialize the "full icon map" with the standalone icons and their paths:
        let mut full_icon_map: HashMap<_, _> = self
            .standalone_icons
            .iter()
            .map(|ico| (ico.icon_name().to_owned(), vec![ico.clone()]))
            .collect();

        // Now, for each theme directory, add every file in it with a supported file extension
        // to the map:
        for path in self.themes_directories.values().flatten() {
            for entry in walkdir::WalkDir::new(path).follow_links(true).into_iter().flatten() {
                // Directories are not icons.
                if entry.file_type().is_dir() {
                    continue;
                }

                let path = entry.into_path();
                let Some(icon) = IconFile::from_path_buf(path) else {
                    // This file was not a valid icon.
                    continue;
                };

                let icons = full_icon_map
                    .entry(icon.icon_name().to_owned())
                    .or_insert_with(Default::default);

                icons.push(icon);
            }
        }

        self.full_icon_map = Some(full_icon_map);
        self.full_icon_map.as_ref().unwrap()
    }

    /// Collects all standalone icons, themes, and all the dependencies of the themes found.
    ///
    /// Wraps everything up into the central [Icons] struct, which may then be used to perform actual
    /// icon lookups.
    pub fn icons(self) -> Icons {
        let themes = self.resolve();

        let standalone_icons = self
            .standalone_icons
            .into_iter()
            .map(|file| {
                let key = file
                    .path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(String::new());
                (key, file)
            })
            .collect();

        Icons {
            standalone_icons,
            themes,
        }
    }

    /// Resolve all themes found in the directories searched, returning a map of internal
    /// theme names to [Theme]s you may use to find icons.
    ///
    /// *Resolving* in this context means:
    /// - For each theme, finding the appropriate theme index file (see [ThemeInfo] and [ThemeIndex](crate::theme::ThemeIndex))
    /// - Find all (transitive) dependencies of themes, performing the same operation(s) for them, and
    /// - Pruning duplicate references in the dependency graph: after `resolve`, each theme has a
    ///   _direct acyclic graph_ of its dependents computed.
    pub fn resolve(&self) -> HashMap<OsString, Arc<Theme>> {
        self.resolve_only(self.themes_directories.keys())
    }

    /// Like [resolve](Self::resolve), but with a restricted set of themes to resolve.
    ///
    /// This still collects all dependencies of the icon themes: for example, \
    /// - The `Adwaita` icon theme inherits `AdwaitaLegacy` and `hicolor`,
    /// - The `AdwaitaLegacy` theme inherits `hicolor`,
    /// - And `hicolor` inherits no other theme.
    ///
    /// Thus, a call to `resolve_only(&["Adwaita"])` will still return a map with `Adwaita`,
    ///   `AdwaitaLegacy` and `hicolor`.
    pub fn resolve_only<I, S>(&self, theme_names: I) -> HashMap<OsString, Arc<Theme>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // Icon themes may transitively depend on the same icon theme many times.
        // This is a bit of an issue, as when an exhaustive icon lookup would be implemented naively,
        // users may end up searching the same icon theme multiple times.
        // To accommodate this, either one has to keep a list of visited icon themes every time they
        // perform a lookup, or avoid the issue altogether by removing redundant parents up-front.

        // That second option is what this function does, being to pay a (rather small) one-time cost to
        // make the rest of the API cleaner and smaller. It guarantees that the returned icon themes
        // have dependencies that form a direct acyclic graph without redundant paths.

        fn collect_themes(
            name: &OsStr,
            locations: &IconLocations,
            themes: &mut HashMap<OsString, Option<ThemeInfo>>,
        ) {
            // Skip if we already have this theme.
            if themes.contains_key(name) {
                return;
            }

            #[allow(clippy::manual_ok_err)] // clippy doesn't see the #[cfg]
            let info = match locations.load_single_theme(name) {
                Ok(d) => Some(d),
                Err(_e) => {
                    #[cfg(feature = "log")]
                    log::debug!("skipping theme candidate {name:?} because {_e}");

                    None
                }
            };
            let info = themes.entry(name.to_os_string()).insert_entry(info);

            let Some(info) = info.get() else {
                return;
            };

            let parents = info.index.inherits.clone();

            // Collect all parents of this theme:
            for parent in parents {
                collect_themes(parent.as_ref(), locations, themes);
            }
        }

        // Map from theme names to their info:
        let mut themes = HashMap::new();

        // collect all required themes:
        for theme_name in theme_names {
            let theme_name = theme_name.as_ref();
            collect_themes(theme_name, self, &mut themes);
        }

        // make 100% sure we have `hicolor`, for the half-impossible edge-case of only collecting
        // themes that does not have hicolor in their inheritance tree
        collect_themes("hicolor".as_ref(), self, &mut themes);
        // of course, the user might be cursed and not have `hicolor` installed at all!
        // that is troubling, but we'll see that it is handled correctly below.

        // let's prune theme candidates that have no info (meaning they weren't themes, or
        //  were invalid)
        // we'll also split them up, as `theme_chains` borrows names from `theme_names`,
        // but we need to mutate theme_info later (during the borrow) to avoid
        // cloning the info
        let (theme_names, mut theme_info): (Vec<_>, Vec<_>) = themes
            .into_iter()
            .flat_map(|(key, value)| value.map(|v| (key, Some(v))))
            .unzip();

        // the Options are there just so we can take info out of the vec without messing up the order.
        debug_assert!(theme_info.iter().all(Option::is_some));

        // do we even have hicolor?
        // if not, there's no use in inserting hicolor into the inheritance tree later
        let hicolor_idx = theme_names.iter().position(|name| name == "hicolor");

        // Time to find the optimal ancestry for each theme.
        // As hicolor _should_ have all icons by default, and all themes depend on hicolor at some depth,
        // DFS would de facto end up in hicolor before ever trying the second theme in an Inherits set.
        // Therefore, BFS is the only sensible option, but the spec doesn't define this.

        // indexed by the position in our theme_names/theme_info vecs
        let number_of_themes = theme_names.len();
        let mut theme_chains = Vec::<Vec<usize>>::with_capacity(number_of_themes);

        for theme_idx in 0..number_of_themes {
            let mut chain = Vec::from([theme_idx]);

            let mut cursor = 0;
            while let Some(node_idx) = chain.get(cursor).copied() {
                cursor += 1;

                let Some(Some(info)) = theme_info.get(node_idx) else {
                    continue;
                };

                for parent in &info.index.inherits {
                    let Some(parent_idx) = theme_names
                        .iter()
                        .position(|name| *name.as_os_str() == **parent)
                    else {
                        // this parent was invalid
                        continue;
                    };

                    // add this parent, removing any previous occurrences
                    chain.retain(|idx| *idx != parent_idx);
                    chain.push(parent_idx);
                }
            }

            // From the spec: "If no theme is specified, implementations are required to add the
            //                 "hicolor" theme to the inheritance tree."
            if let Some(hicolor_idx) = hicolor_idx {
                chain.retain(|idx| *idx != hicolor_idx);
                chain.push(hicolor_idx);
            }

            theme_chains.push(chain);
        }

        // at this point `theme_chains` contains a _topological order_ for each theme's parents,
        // meaning we can easily iterate over it, constructing `Theme`s, assuming at every point
        // that each parent already has a `Theme` created for it :)

        // again indexed by theme indices, None values mean the theme hasn't been processed yet.
        // the goal is that, by the end of the for loop, that this only contains `Some`s.
        // we rely on the topological order of chains to always have all the prerequisite themes
        // present already in this map!
        let mut full_themes = vec![None::<Arc<Theme>>; number_of_themes];

        for chain in &theme_chains {
            // go from last theme to first, as all dependencies are "forward" in the chain:
            for theme_idx in chain.iter().copied().rev() {
                let theme_info = theme_info[theme_idx].take();

                let Some(theme_info) = theme_info else {
                    // the option was None, meaning this theme was processed already :-)
                    continue;
                };

                let parents = &theme_chains[theme_idx];
                let parents = parents
                    .iter()
                    .skip(1) // the first in the chain is the theme itself, which we'll ignore—it's not a parent.
                    .copied()
                    // unwrap OK because, by the topological order, all of these parents
                    // should already be present in the array:
                    .map(|parent_idx| Arc::clone(full_themes[parent_idx].as_ref().unwrap()))
                    .collect();

                let theme = Theme {
                    info: theme_info,
                    inherits_from: parents,
                };

                full_themes[theme_idx] = Some(Arc::new(theme));
            }
        }

        debug_assert!(full_themes.iter().all(Option::is_some));

        let full_themes: Vec<_> = full_themes.into_iter().map(Option::unwrap).collect();

        // and so, we have reached the end of the Big Beautiful Function.
        // `full_themes` is a list of
        // - All themes requested,
        // - all themes required by the inheritance tree of those themes, without duplicates,
        // - and an optimal chain (inheritance tree search order) for each theme.

        // and to wrap things up, let's zip the themes back up with their names
        theme_names
            .into_iter()
            .zip(full_themes)
            .collect::<HashMap<_, _>>()
    }

    /// Parse a single theme, returning its info.
    ///
    /// This is a rather low-level function, as it does not give you (easy) access to a usable
    /// version of the theme's inheritance tree.
    ///
    /// Unless theme metadata is all you need, use [`resolve`](IconLocations::resolve) or [`resolve_only`](IconLocations::resolve_only) instead!
    pub fn load_single_theme<S>(&self, internal_name: S) -> std::io::Result<ThemeInfo>
    where
        S: AsRef<OsStr>,
    {
        let internal_name = internal_name.as_ref();

        let theme = self
            .themes_directories
            .get(internal_name)
            .ok_or_else(|| std::io::Error::other(ThemeParseError::NotAnIconTheme))?;

        ThemeInfo::new_from_folders(internal_name.to_string_lossy().into_owned(), theme.clone())
    }

    /// Look up a standalone icon by name.
    ///
    /// "Standalone" icons are icons that live outside icon themes, residing at the root in the
    /// search directories instead. These icons do not have any size or scalability information attached to them.
    ///
    /// This function exists for use cases where you don't need theme information, but keep in mind
    /// that its counterpart in [Icons]: [Icons::find_standalone_icon] is usually used instead.
    pub fn standalone_icon<S>(&self, icon_name: S) -> Option<&IconFile>
    where
        S: AsRef<OsStr>,
    {
        let name = icon_name.as_ref();

        self.standalone_icons
            .iter()
            .find(|icon| icon.path().file_stem() == Some(name))
    }
}

/// Anything that turns into an iterator of things that can become paths can be turned into an [`IconSearch`].
impl<I, P> From<I> for IconSearch
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    fn from(value: I) -> Self {
        let dirs = value.into_iter().map(Into::into).collect();

        IconSearch::new_from(dirs)
    }
}

impl Default for IconSearch {
    fn default() -> Self {
        // "By default, apps should look in $HOME/.icons (for backwards compatibility),
        // in $XDG_DATA_DIRS/icons
        // and in /usr/share/pixmaps (in that order)."

        let xdg = xdg::BaseDirectories::new();

        let mut directories = vec![];

        if let Some(home) = std::env::home_dir() {
            directories.push(home.join(".icons"));
        }

        xdg.data_home
            .into_iter()
            .chain(xdg.data_dirs)
            .map(|data_dir| data_dir.join("icons"))
            .for_each(|dir| directories.push(dir));

        directories.push("/usr/share/pixmaps".into());

        directories.into()
    }
}

#[cfg(test)]
mod test {
    use crate::search::IconSearch;
    use std::collections::HashSet;
    use std::path::PathBuf;

    static PROJ_ROOT: &'static str = env!("CARGO_MANIFEST_DIR");

    fn test_search() -> IconSearch {
        IconSearch::new_empty().add_directories([
            PathBuf::from(PROJ_ROOT).join("resources/test_icons"),
            PathBuf::from(PROJ_ROOT).join("resources/test_icons_alt"),
        ])
    }

    // these tests assume certain applications are installed on the system they are run on.

    #[test]
    fn test_standard_usage() {
        let icons = test_search()
            .add_directories(["/this/path/probably/doesnt/exist/but/who/cares/"])
            .search();

        assert!(
            icons
                .dirs
                .contains(&"/this/path/probably/doesnt/exist/but/who/cares/".into()),
            "IconSearch contains the added directory"
        );

        // Two themes in the testing directories: TestTheme and OtherTheme
        assert_eq!(
            icons.icon_locations().themes_directories.len(),
            2
        );

        let _ = icons.icons();

        // no panic
    }

    #[test]
    fn test_find_test_theme() {
        let dirs = test_search();
        let locations = dirs.find_icon_locations();

        let theme = locations.load_single_theme("TestTheme").unwrap();

        assert_eq!(theme.internal_name, "TestTheme", "name is correct");
        assert_eq!(
            theme.index.comment,
            "This is a theme to test icon's capabilities."
        );
        assert_eq!(theme.index.hidden, true);
        assert_eq!(
            theme.base_dirs.len(),
            2,
            "correct amount of base directories"
        );
        assert_eq!(theme.index_location.file_name().unwrap(), "index.theme");
        assert_eq!(theme.index.name, "HelloTestTheme!");
        assert_eq!(
            theme.index.comment,
            "This is a theme to test icon's capabilities."
        );
        assert_eq!(theme.index.inherits, vec![String::from("OtherTheme")]);

        let directories = theme.index.directories.iter();

        assert_eq!(
            directories
                .clone()
                .map(|di| di.directory_name.as_str())
                .collect::<HashSet<_>>(),
            [
                "16x16/α",
                "16x16/β",
                "32x32/foo",
                "UnconventionalDirectoryName/γ",
                "128x128"
            ]
            .into()
        );

        assert_eq!(
            directories
                .clone()
                .map(|di| di.size)
                .collect::<HashSet<_>>(),
            [16, 32, 64, 128].into()
        );
    }

    #[test]
    #[cfg(feature = "full-search")]
    fn test_full_icon_search() {
        let mut icon_locations = test_search().find_icon_locations();
        let map = icon_locations.full_icon_search();

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
