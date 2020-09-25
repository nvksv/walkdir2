//use crate::error::{into_io_err, Error};
use crate::fs::{self, FsFileType, FsRootDirEntry, FsMetadata};
use crate::wd::{self, Depth, IntoSome, IntoOk};
use crate::cp::ContentProcessor;

use std::vec::Vec;

/////////////////////////////////////////////////////////////////////////////////

/// A directory entry.
///
/// This is the type of value that is yielded from the iterators defined in
/// this crate.
///
/// On Unix systems, this type implements the [`DirEntryExt`] trait, which
/// provides efficient access to the inode number of the directory entry.
///
/// # Differences with `std::fs::DirEntry`
///
/// This type mostly mirrors the type by the same name in [`std::fs`]. There
/// are some differences however:
///
/// * All recursive directory iterators must inspect the entry's type.
/// Therefore, the value is stored and its access is guaranteed to be cheap and
/// successful.
/// * [`path`] and [`file_name`] return borrowed variants.
/// * If [`follow_links`] was enabled on the originating iterator, then all
/// operations except for [`path`] operate on the link target. Otherwise, all
/// operations operate on the symbolic link.
///
/// [`std::fs`]: https://doc.rust-lang.org/stable/std/fs/index.html
/// [`path`]: #method.path
/// [`file_name`]: #method.file_name
/// [`follow_links`]: struct.WalkDir.html#method.follow_links
/// [`DirEntryExt`]: trait.DirEntryExt.html
#[derive(Debug)]
pub struct DirEntry<E: fs::FsDirEntry = fs::DefaultDirEntry> {
    /// Raw dent
    path: E::PathBuf,
    /// Follow link
    follow_link: bool,
    /// Is normal dir
    is_dir: bool,
    /// Cached metadata
    metadata: E::Metadata,
    /// Cached file name
    file_name: E::FileName,
    /// The depth at which this entry was generated relative to the root.
    depth: Depth,
}

impl<E: fs::FsDirEntry> Clone for DirEntry<E> {
    fn clone(&self) -> Self {
        Self {
            path:           self.path.clone(),
            follow_link:    self.follow_link,
            is_dir:         self.is_dir,
            metadata:       self.metadata.clone(),
            file_name:      self.file_name.clone(),
            depth:          self.depth,
        }    
    }
}

impl<E: fs::FsDirEntry> DirEntry<E> {
    /// The full path that this entry represents.
    ///
    /// The full path is created by joining the parents of this entry up to the
    /// root initially given to [`WalkDir::new`] with the file name of this
    /// entry.
    ///
    /// Note that this *always* returns the path reported by the underlying
    /// directory entry, even when symbolic links are followed. To get the
    /// target path, use [`path_is_symlink`] to (cheaply) check if this entry
    /// corresponds to a symbolic link, and [`std::fs::read_link`] to resolve
    /// the target.
    ///
    /// [`WalkDir::new`]: struct.WalkDir.html#method.new
    /// [`path_is_symlink`]: struct.DirEntry.html#method.path_is_symlink
    /// [`std::fs::read_link`]: https://doc.rust-lang.org/stable/std/fs/fn.read_link.html
    pub fn path(&self) -> &E::Path {
        &self.path
    }

    /// The full path that this entry represents.
    ///
    /// Analogous to [`path`], but moves ownership of the path.
    ///
    /// [`path`]: struct.DirEntry.html#method.path
    pub fn into_path(self) -> E::PathBuf {
        self.path
    }

    /// Returns `true` if and only if this entry was created from a symbolic
    /// link. This is unaffected by the [`follow_links`] setting.
    ///
    /// When `true`, the value returned by the [`path`] method is a
    /// symbolic link name. To get the full target path, you must call
    /// [`std::fs::read_link(entry.path())`].
    ///
    /// [`path`]: struct.DirEntry.html#method.path
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    /// [`std::fs::read_link(entry.path())`]: https://doc.rust-lang.org/stable/std/fs/fn.read_link.html
    pub fn path_is_symlink(&self) -> bool {
        self.metadata.file_type().is_symlink() || self.follow_link
    }

    /// Return the metadata for the file that this entry points to.
    ///
    /// This will follow symbolic links if and only if the [`WalkDir`] value
    /// has [`follow_links`] enabled.
    ///
    /// # Platform behavior
    ///
    /// This always calls [`std::fs::symlink_metadata`].
    ///
    /// If this entry is a symbolic link and [`follow_links`] is enabled, then
    /// [`std::fs::metadata`] is called instead.
    ///
    /// # Errors
    ///
    /// Similar to [`std::fs::metadata`], returns errors for path values that
    /// the program does not have permissions to access or if the path does not
    /// exist.
    ///
    /// [`WalkDir`]: struct.WalkDir.html
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    /// [`std::fs::metadata`]: https://doc.rust-lang.org/std/fs/fn.metadata.html
    /// [`std::fs::symlink_metadata`]: https://doc.rust-lang.org/stable/std/fs/fn.symlink_metadata.html
    pub fn metadata(&self) -> wd::Result<E::Metadata, E> {
        self.metadata.clone().into_ok()
    }

    /// Return the file type for the file that this entry points to.
    ///
    /// If this is a symbolic link and [`follow_links`] is `true`, then this
    /// returns the type of the target.
    ///
    /// This never makes any system calls.
    ///
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    pub fn file_type(&self) -> E::FileType {
        self.metadata.file_type()
    }

    /// Return the file name of this entry.
    ///
    /// If this entry has no file name (e.g., `/`), then the full path is
    /// returned.
    pub fn file_name(&self) -> &E::FileName {
        &self.file_name
    }

    /// Returns the depth at which this entry was created relative to the root.
    ///
    /// The smallest depth is `0` and always corresponds to the path given
    /// to the `new` function on `WalkDir`. Its direct descendents have depth
    /// `1`, and their descendents have depth `2`, and so on.
    pub fn depth(&self) -> Depth {
        self.depth
    }

    /////////////////////////////////////////////////////////////////////////////////
    
    /// Returns true if and only if this entry points to a directory.
    pub(crate) fn is_dir(&self) -> bool {
        self.is_dir
    }
}

// /////////////////////////////////////////////////////////////////////////////////

// /// Unix-specific extension methods for `walkdir::DirEntry`
// #[cfg(unix)]
// pub trait DirEntryExt {
//     /// Returns the underlying `d_ino` field in the contained `dirent`
//     /// structure.
//     fn ino(&self) -> u64;
// }

// #[cfg(unix)]
// impl DirEntryExt for DirEntry<fs::UnixDirEntry> {
//     /// Returns the underlying `d_ino` field in the contained `dirent`
//     /// structure.
//     fn ino(&self) -> u64 {
//         self.inner().ino
//     }
// }

/////////////////////////////////////////////////////////////////////////////////


/// Convertor from RawDirEntry into DirEntry
#[derive(Debug, Default)]
pub struct DirEntryContentProcessor {}

impl<E: fs::FsDirEntry> ContentProcessor<E> for DirEntryContentProcessor {
    type Item = DirEntry<E>;
    type Collection = Vec<DirEntry<E>>;

    /// Convert RawDirEntry into final entry type (e.g. DirEntry)
    fn process_root_direntry(
        &self,
        fsdent: &mut E::RootDirEntry,
        follow_link: bool,
        is_dir: bool,
        depth: Depth,
        ctx: &mut E::Context,
    ) -> Option<Self::Item> {
        let (path, metadata, file_name) = fsdent.to_parts( follow_link, true, true, ctx ); 

        Self::Item {
            path,
            follow_link,
            is_dir,
            metadata: metadata.unwrap(),
            file_name: file_name.unwrap(),
            depth,
        }.into_some()
    }

    /// Convert RawDirEntry into final entry type (e.g. DirEntry)
    fn process_direntry(
        &self,
        fsdent: &mut E,
        follow_link: bool,
        is_dir: bool,
        depth: Depth,
        ctx: &mut E::Context,
    ) -> Option<Self::Item> {
        let (path, metadata, file_name) = fsdent.to_parts( follow_link, true, true, ctx ); 

        Self::Item {
            path,
            follow_link,
            is_dir,
            metadata: metadata.unwrap(),
            file_name: file_name.unwrap(),
            depth,
        }.into_some()
    }

    /// Check if final entry is dir
    fn is_dir(item: &Self::Item) -> bool {
        item.is_dir()
    }

    /// Collects iterator over items into collection
    fn collect(&self, iter: impl Iterator<Item = Self::Item>) -> Self::Collection {
        iter.collect()
    }
    /// Empty items collection
    fn empty_collection() -> Self::Collection {
        vec![]
    }

}
