mod dent;

use crate::fs;
use crate::wd::Depth;

pub use dent::{DirEntry, DirEntryContentProcessor};

use std::iter::FromIterator;

/// Convertor from RawDirEntry into final entry type (e.g. DirEntry)
pub trait ContentProcessor<E: fs::FsDirEntry>: std::fmt::Debug {
    /// Final entry type
    type Item;
    /// Collection of items
    type Collection: FromIterator<Self::Item>;

    /// Convert RawDirEntry into final entry type (e.g. DirEntry)
    fn process_root_direntry(
        &self,
        fsdent: &mut E::RootDirEntry,
        follow_link: bool,
        is_dir: bool,
        depth: Depth,
        ctx: &mut E::Context,
    ) -> Option<Self::Item>;

    /// Convert RawDirEntry into final entry type (e.g. DirEntry)
    fn process_direntry(
        &self,
        fsdent: &mut E,
        follow_link: bool,
        is_dir: bool,
        depth: Depth,
        ctx: &mut E::Context,
    ) -> Option<Self::Item>;

    /// Check if final entry is dir
    fn is_dir(item: &Self::Item) -> bool;

    /// Collects iterator over items into collection
    fn collect(&self, iter: impl Iterator<Item = Self::Item>) -> Self::Collection;
    /// Empty items collection
    fn empty_collection() -> Self::Collection;
}

