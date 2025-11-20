#![warn(missing_docs)]

//! Turns out finding icons correctly on linux is kind of hard.
//!
//! This crate, `icon`, implements the XDG icon theme specification fully, while (hopefully) giving maximum coverage in usecases without sacrificing speed or compliance.
//!
//! # Quick start
//!
//! ```
//! // Find all icon themes using the standard locations.
//! let icons = icon::Icons::new();
//!
//! // Find an icon named "firefox" with size 128x128 in theme "Adwaita"
//! let firefox: Option<icon::IconFile> = icons.find_icon("firefox", 128, 1, "Adwaita");
//!
//! println!("Firefox icon is at {:?}", firefox.unwrap().path())
//! ```
//!
//! See [Icons].
//!
//! # Icon matching
//!
//! Matching a desired size and scale for an icon to the actual file for that size and scale isn't
//! as simple as you'd hope for it to be.
//!
//! Concretely, icon themes organize icons into subdirectories that have a number of properties
//! that specify how the icons in them are allowed to be used:
//! - Directories can contain scalable graphics (SVGs), and specify a minimum and maximum pixel size
//!   at which those graphics may be used. For example, an icon theme may have simplified SVGs for
//!   displaying small icons, and more complex SVGs for when the icons are larger.
//! - In case an odd size is requested, for example 30x30px (a size not present in most icon themes),
//!   applications are expected to find the _next best match_ for that size. To accommodate that,
//!   icon directories specify the base size of its icons and a threshold wherein applications can
//!   choose to up- or downscale icons when an exact match to exist. For example, `hicolor`'s 32x32
//!   directory specifies a threshold value of 2, meaning icons from 30x30px to 34x34px may be
//!   displayed using the 32x32px images.
//!
//! `icon` (this crate)'s job is to make finding the **correct icon** for a given query as simple as
//! possible.
//!
//! # High level design
//!
//! Finding icons is a multi-stage procedure, and depending on your use case, you might want to stop
//! doing work at any one of them.
//! This crate is laid out to allow you to do exactly that, befitting those who need to find
//! just one icon in one theme but also those who need a reliable cache of many icons for many themes.
//!
//! In general, the steps are as follows:
//!
//! 1.  *Finding standalone icons and themes*:
//!
//!     Icons are found either in icon themes or 'standalone' (outside a theme) in XDG base directories.
//!     While a number of directories should always be scanned for icons, the user or application is
//!     allowed to search additional directories as it sees fit.
//!
//!     [IconSearch] handles this part, and is also the main entrypoint for `icon`.
//!
//! 2.  *Parsing and resolving icon themes*:
//!
//!     Each icon theme lives in a directory in the root of one or more of the "search directories".
//!     The name of its directory is called the theme's _internal name_, and in it lies the theme's
//!     definition, `index.theme`.
//!
//!     Icon themes also can declare other icon themes from which they inherit. For that reason,
//!     we also need to do the additional work of figuring out the inheritance tree of each theme,
//!     removing (transitive) duplicates from the inheritance tree, etc.
//!
//! 3.  *Finding icons*:
//!
//!     Once a full picture of icons and theme 'graphs' is obtained, we can start looking up icons.
//!
//! # Alternative crates
//!
//! - [linicon](https://crates.io/crates/linicon) also implements icon finding, but:
//!   - it does not scan "standalone" icons correctly, such as those usually found in `/usr/share/pixmaps`.
//!   - it adopts a one-shot approach, repeating all parsing and file-finding work for each icon.
//!   - it does not provide support for caching.
//!
//! - [icon-loader](https://crates.io/crates/icon-loader) also implements icon finding, but:
//!   - like `linicon`, it does not scan "standalone" icons correctly.
//!   - it only supports a rust-native icon cache, which you cannot opt out of.
//!   - it provides only icon loading—you cannot use it to obtain information about Icon Themes.

mod icon;
mod search;
mod theme;

pub use icon::*;
pub use search::*;
pub use theme::*;
