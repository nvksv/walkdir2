/*!
Crate `walkdir` provides an efficient and cross platform implementation
of recursive directory traversal. Several options are exposed to control
iteration, such as whether to follow symbolic links (default off), limit the
maximum number of simultaneous open file descriptors and the ability to
efficiently skip descending into directories.

To use this crate, add `walkdir` as a dependency to your project's
`Cargo.toml`:

```toml
[dependencies]
walkdir2 = "0.1"
```

# From the top

The [`WalkDir`] type builds iterators. The [`DirEntry`] type describes values
yielded by the iterator. Finally, the [`Error`] type is a small wrapper around
[`std::io::Error`] with additional information, such as if a loop was detected
while following symbolic links (not enabled by default).

[`WalkDir`]: struct.WalkDir.html
[`DirEntry`]: struct.DirEntry.html
[`Error`]: struct.Error.html
[`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html

# Example

The following code recursively iterates over the directory given and prints
the path for each entry:

```no_run
use walkdir2::{WalkDir, WalkDirIter, ClassicWalkDirIter};
# use walkdir2::Error;

# fn try_main() -> Result<(), Error> {
for entry in WalkDir::new("foo").into_classic() {
    println!("{}", entry?.path().display());
}
# Ok(())
# }
```

Or, if you'd like to iterate over all entries and ignore any errors that
may arise, use [`filter_map`]. (e.g., This code below will silently skip
directories that the owner of the running process does not have permission to
access.)

```no_run
use walkdir2::{WalkDir, WalkDirIter, ClassicWalkDirIter};

for entry in WalkDir::new("foo").into_classic().filter_map(|e| e.ok()) {
    println!("{}", entry.path().display());
}
```

[`filter_map`]: https://doc.rust-lang.org/stable/std/iter/trait.Iterator.html#method.filter_map

# Example: follow symbolic links

The same code as above, except [`follow_links`] is enabled:

```no_run
use walkdir2::{WalkDir, WalkDirIter, ClassicWalkDirIter};
# use walkdir2::Error;

# fn try_main() -> Result<(), Error> {
for entry in WalkDir::new("foo").follow_links(true).into_classic() {
    println!("{}", entry?.path().display());
}
# Ok(())
# }
```

[`follow_links`]: struct.WalkDir.html#method.follow_links

# Example: skip hidden files and directories on unix

This uses the [`filter_entry`] iterator adapter to avoid yielding hidden files
and directories efficiently (i.e. without recursing into hidden directories):

```no_run
use walkdir2::{DirEntry, WalkDir, WalkDirIter, ClassicWalkDirIter};
# use walkdir2::Error;

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with("."))
         .unwrap_or(false)
}

# fn try_main() -> Result<(), Error> {
let walker = WalkDir::new("foo").into_classic();
for entry in walker.filter_entry(|e| !is_hidden(e)) {
    println!("{}", entry?.path().display());
}
# Ok(())
# }
```

[`filter_entry`]: struct.IntoIter.html#method.filter_entry
*/

#![deny(missing_docs)]
#![allow(unknown_lints)]

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

mod wd;
mod error;
/// Filesystem-specific parts
pub mod fs;
mod walk;
/// Content processor
pub mod cp;

#[cfg(test)]
mod tests;

// pub use crate::dent::DirEntry;
// #[cfg(unix)]
// pub use crate::dent::DirEntryExt;

// pub use crate::classic_iter::ClassicWalkDirIter;
// pub use crate::cp::{ContentProcessor, DirEntryContentProcessor};
// pub use crate::iter::{FilterEntry, WalkDirIter};
// pub use crate::opts::WalkDirBuilder;
// pub use crate::storage::{StoragePath, StoragePathBuf};
// pub use crate::walk::WalkDirIterator;
// pub use crate::wd::{ContentFilter, ContentOrder, Depth, Position, WalkDirIteratorItem};

/// Default (classic) WalkDir
pub type WalkDir = WalkDirBuilder<fs::DefaultDirEntry, cp::DirEntryContentProcessor>;

pub use wd::{ContentFilter, ContentOrder, Depth, FnCmp, Position, Result, ResultInner};
pub use walk::{ClassicFilterEntry, ClassicIter, ClassicWalkDirIter, FilterEntry, RawDirEntry, ReadDir, WalkDirBuilder, WalkDirIter, WalkDirIterator, WalkDirIteratorItem, WalkDirOptions};
pub use error::Error;
pub use cp::DirEntry;


