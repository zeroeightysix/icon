//! Turns out finding icons correctly on linux is kind of hard.
//!
//! This crate, `icon`, implements the XDG icon theme specification fully, while (hopefully) giving maximum coverage in usecases without sacrificing speed or compliance.
//!
//! # Quick start
//!
//! ```
//! let icons = icon::Icons::new();
//!
//! let firefox: Option<icon::IconFile> = icons.find_icon("firefox", 128, 1, "Adwaita");
//!
//! println!("Firefox icon is at {:?}", firefox.unwrap().path)
//! ```
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