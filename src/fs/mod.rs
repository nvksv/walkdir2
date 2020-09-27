use std::ops::Deref;
use std::fmt::Debug;

mod path;
mod standard;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use crate::wd::{IntoSome, IntoErr};
pub use self::path::{FsPath, FsPathBuf};
pub use self::standard::{StandardDirEntry, StandardDirFingerprint, StandardReadDir, StandardRootDirEntry};

#[cfg(unix)]
pub use self::unix::{UnixDirEntry, UnixReadDir, UnixRootDirEntry};
#[cfg(windows)]
pub use self::windows::{WindowsDirEntry, WindowsReadDir, WindowsRootDirEntry};

#[cfg(not(any(unix, windows)))]
/// Default storage-specific type.
pub type DefaultDirEntry = StandardDirEntry;
#[cfg(unix)]
/// Default source-specific type.
pub type DefaultDirEntry = UnixDirEntry;
#[cfg(windows)]
/// Default source-specific type.
pub type DefaultDirEntry = WindowsDirEntry;


///////////////////////////////////////////////////////////////////////////////////////////////

/// Functions for FsMetadata
pub trait FsError: 'static + std::error::Error + Debug {
    /// Inner error type
    type Inner;

    /// Creates a new I/O error from a known kind of error as well as an arbitrary error payload.
    fn from_inner(error: Self::Inner) -> Self;
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// Functions for FsFileType
pub trait FsFileType: Clone + Copy + Debug {
    /// Is it dir?
    fn is_dir(&self) -> bool;
    /// Is it file
    fn is_file(&self) -> bool;
    /// Is it symlink
    fn is_symlink(&self) -> bool;
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// Functions for FsMetadata
pub trait FsMetadata: Debug + Clone {
    /// Associated FileType type
    type FileType: FsFileType;

    /// Get type of this entry
    fn file_type(&self) -> Self::FileType;
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// Iterator over FsReadDir
pub trait FsReadDirIterator: Debug + Sized {
    /// Associated fs context
    type Context: Debug;

    /// Associated error type
    type Error;
    /// Associated FsDirEntry implementation type
    type DirEntry;

    /// Get next dir entry
    fn next_entry(
        &mut self, 
        ctx: &mut Self::Context,
    ) -> Option<Result<Self::DirEntry, Self::Error>>;
}

/// Functions for FsReadDir
pub trait FsReadDir: Debug + Sized {
    /// Associated fs context
    type Context:   Debug;
    /// Underlying ReadDir object type
    type Inner:     FsReadDirIterator<Context = Self::Context>;
    /// Associated error type
    type Error:     FsError<Inner = <Self::Inner as FsReadDirIterator>::Error>;
    /// Associated FsDirEntry implementation type
    type DirEntry:  FsDirEntry<Context = Self::Context, Error = Self::Error>;

    /// Get inner ReadDir object
    fn inner_mut(&mut self) -> &mut Self::Inner;
    /// Convert inner DirEntry object into associated DirEntry
    fn process_inner_entry(&mut self, inner_entry: <Self::Inner as FsReadDirIterator>::DirEntry) -> Result<Self::DirEntry, Self::Error>;

    /// Iterate over dir content
    fn next_fsentry(
        &mut self,
        ctx: &mut Self::Context,
    ) -> Option<Result<Self::DirEntry, Self::Error>> {
        match self.inner_mut().next_entry(ctx)? {
            Ok(inner_entry) => self.process_inner_entry(inner_entry),
            Err(err)        => Self::Error::from_inner(err).into_err(),
        }.into_some()
    }
}

impl<RD> FsReadDirIterator for RD where RD: FsReadDir {
    type Context    = RD::Context;
    type Error      = RD::Error;
    type DirEntry   = RD::DirEntry;

    fn next_entry(
        &mut self,
        ctx: &mut Self::Context,
    ) -> Option<Result<Self::DirEntry, Self::Error>> {
        self.next_fsentry(ctx)
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// Functions for FsDirEntry
pub trait FsDirEntry: Debug + Sized {
    /// Associated fs context
    type Context:   Debug;

    /// Path type (unsized)
    type Path:      FsPath<PathBuf = Self::PathBuf, FileName = Self::FileName> + AsRef<Self::Path> + ?Sized;
    /// Owned path type
    type PathBuf:   for<'p> FsPathBuf<'p> + AsRef<Self::Path> + Deref<Target = Self::Path> + Sized;
    /// Owned file name type
    type FileName:  Sized + Debug + Clone;

    /// Error type
    type Error:             FsError;
    /// FileType type
    type FileType:          FsFileType;
    /// Metadata type
    type Metadata:          FsMetadata<FileType=Self::FileType>;
    /// FsReadDir implementation object type
    type ReadDir:           FsReadDirIterator<Context=Self::Context, DirEntry=Self, Error=Self::Error>;
    /// Fingerprint type
    type DirFingerprint:    Debug + Eq;
    /// Device num type
    type DeviceNum:         Debug + Eq + Clone + Copy;
    /// FsRootReadDir implementation object type
    type RootDirEntry:      FsRootDirEntry<Context=Self::Context, DirEntry=Self>;

    /// Get path of this entry
    fn path(&self) -> &Self::Path;
    /// Get path of this entry
    fn pathbuf(&self) -> Self::PathBuf;
    /// Get canonical path of this entry (don't follow symlink!)
    fn canonicalize(&self) -> Result<Self::PathBuf, Self::Error>;
    /// Get bare name of this entry withot any leading path components (don't follow symlink!)
    fn file_name(&self) -> Self::FileName;

    /// Get file type
    fn file_type(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<Self::FileType, Self::Error>;

    /// Get metadata
    fn metadata(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<Self::Metadata, Self::Error>;

    /// Read dir (always follow symlink!)
    fn read_dir(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::ReadDir, Self::Error>;

    /// Return the unique handle (always follow symlink!)
    fn fingerprint(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::DirFingerprint, Self::Error>;

    /// Compare two dirs for sameness
    fn is_same(
        lhs: (&Self::Path, &Self::DirFingerprint),
        rhs: (&Self::Path, &Self::DirFingerprint),
    ) -> bool;

    /// device_num (always follow symlink!)
    fn device_num(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::DeviceNum, Self::Error>;

    /// Get cached metadata (if exists)
    fn to_parts(
        &mut self,
        follow_link: bool,
        force_metadata: bool,
        force_file_name: bool,
        ctx: &mut Self::Context,
    ) -> (Self::PathBuf, Option<Self::Metadata>, Option<Self::FileName>);
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// Functions for FsRootDirEntry
pub trait FsRootDirEntry: Debug + Sized {
    /// Associated fs context
    type Context:   Debug;
    /// Associated FsDirEntry implementation type
    type DirEntry:  FsDirEntry<Context=Self::Context, RootDirEntry=Self>;

    /// Get path of this entry
    fn path(&self) -> &<Self::DirEntry as FsDirEntry>::Path;
    /// Get path of this entry
    fn pathbuf(&self) -> <Self::DirEntry as FsDirEntry>::PathBuf;
    /// Get canonical path of this entry
    fn canonicalize(&self) -> Result<<Self::DirEntry as FsDirEntry>::PathBuf, <Self::DirEntry as FsDirEntry>::Error>;
    /// Get bare name of this entry withot any leading path components
    fn file_name(&self) -> <Self::DirEntry as FsDirEntry>::FileName;

    /// Create new root dir entry object from path
    fn from_path(
        path: &<Self::DirEntry as FsDirEntry>::Path,
        ctx: &mut Self::Context,
    ) -> Result<Self, <Self::DirEntry as FsDirEntry>::Error>;

    /// Get file type
    fn file_type(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::FileType, <Self::DirEntry as FsDirEntry>::Error>;

    /// Get metadata
    fn metadata(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::Metadata, <Self::DirEntry as FsDirEntry>::Error>;

    /// Read dir
    fn read_dir(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::ReadDir, <Self::DirEntry as FsDirEntry>::Error>;

    /// Return the unique handle
    fn fingerprint(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::DirFingerprint, <Self::DirEntry as FsDirEntry>::Error>;

    /// device_num
    fn device_num(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::DeviceNum, <Self::DirEntry as FsDirEntry>::Error>;

    /// Get cached metadata (if exists)
    fn to_parts(
        &mut self,
        follow_link: bool,
        force_metadata: bool,
        force_file_name: bool,
        ctx: &mut Self::Context,
    ) -> (<Self::DirEntry as FsDirEntry>::PathBuf, Option<<Self::DirEntry as FsDirEntry>::Metadata>, Option<<Self::DirEntry as FsDirEntry>::FileName>);
}
