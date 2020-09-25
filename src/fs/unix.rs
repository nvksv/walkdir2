// use crate::storage::{Nil, StorageExt};

// use std::fmt::Debug;
// use std::fs;
// use std::io;
// use std::path;

// use same_file;

// use crate::dent::DirEntry;

// #[derive(Debug, Clone)]
// pub struct RawDirEntryUnixExt {
//     /// The underlying inode number (Unix only).
//     pub(crate) ino: u64,
// }

// /// Unix-specific extensions
// #[derive(Debug, Clone)]
// pub struct WalkDirUnixExt {}

// impl StorageExt for WalkDirUnixExt {
//     type BuilderCtx = Nil;

//     type OptionsExt = Nil;
//     type IteratorExt = Nil;
//     type AncestorExt = Nil;
//     type DirEntryExt = DirEntryUnixExt;
//     type RawDirEntryExt = DirEntryUnixExt;

//     type FsError = std::io::Error;
//     type FsFileName = std::ffi::OsStr;
//     type FsDirEntry = std::fs::DirEntry;
//     type FsReadDir = std::fs::ReadDir;
//     type FsFileType = std::fs::FileType;
//     type FsMetadata = std::fs::Metadata;

//     type Path = path::Path;
//     type PathBuf = path::PathBuf;

//     type SameFileHandle = same_file::Handle;

//     /// Make new builder
//     #[allow(unused_variables)]
//     fn builder_new<P: AsRef<Self::Path>>(root: P, ctx: Option<Self::BuilderCtx>) -> Self {
//         Self {}
//     }

//     /// Make new ancestor
//     fn ancestor_new(dent: &Self::DirEntry) -> Result<Self::AncestorExt, Self::Error> {
//         Ok(Self::AncestorExt {})
//     }

//     #[allow(unused_variables)]
//     fn iterator_new(self) -> Self::IteratorExt {
//         Self::IteratorExt {}
//     }

//     /// Create extension from DirEntry
//     fn rawdent_from_fsentry(ent: &Self::DirEntry) -> Result<Self::RawDirEntryExt, Self::Error> {
//         (Self::RawDirEntryExt { ino: ent.ino() }).into_ok()
//     }

//     /// Create extension from metadata
//     fn rawdent_from_path<P: AsRef<Self::Path>>(
//         path: P,
//         follow_link: bool,
//         md: Self::Metadata,
//         ctx: &mut Self::IteratorExt,
//     ) -> Result<Self::RawDirEntryExt, Self::Error> {
//         Self::RawDirEntryExt { ino: md.ino() }
//     }

//     fn metadata<P: AsRef<Self::Path>>(
//         path: P,
//         follow_link: bool,
//         raw_ext: Option<&Self::RawDirEntryExt>,
//         ctx: &mut Self::IteratorExt,
//     ) -> Result<Self::Metadata, Self::Error> {
//         if follow_link {
//             fs::metadata(path)
//         } else {
//             fs::symlink_metadata(path)
//         }
//     }

//     #[allow(unused_variables)]
//     fn read_dir<P: AsRef<Self::Path>>(
//         path: P,
//         raw_ext: &Self::RawDirEntryExt,
//         ctx: &mut Self::IteratorExt,
//     ) -> Result<Self::ReadDir, Self::Error> {
//         fs::read_dir(path.as_ref())
//     }

//     fn get_handle<P: AsRef<Self::Path>>(path: P) -> io::Result<Self::SameFileHandle> {
//         same_file::Handle::from_path(path)
//     }

//     #[allow(unused_variables)]
//     fn is_same(
//         ancestor_path: &Self::PathBuf,
//         ancestor_ext: &Self::AncestorExt,
//         child: &Self::SameFileHandle,
//     ) -> io::Result<bool> {
//         Ok(child == &Self::get_handle(ancestor_path)?)
//     }

//     #[allow(unused_variables)]
//     fn dent_from_rawdent(raw: &Self::RawDirEntryExt) -> Self::DirEntryExt {
//         raw
//     }

//     fn device_num<P: AsRef<Self::Path>>(path: P) -> io::Result<u64> {
//         use std::os::unix::fs::MetadataExt;

//         path.as_ref().metadata().map(|md| md.dev())
//     }

//     fn get_file_name(path: &Self::PathBuf) -> &Self::FileName {
//         path.file_name().unwrap_or_else(|| path.as_os_str())
//     }
// }


use crate::fs::standard::{StandardDirEntry, StandardReadDir, StandardRootDirEntry};
use crate::fs::{FsDirEntry, FsReadDir, FsRootDirEntry};
use crate::wd::IntoOk;

use std::fmt::Debug;

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Unix FsReadDir implementation using std::fs::* objects 
#[derive(Debug)]
pub struct UnixReadDir {
    standard: StandardReadDir,
}

impl UnixReadDir {
    /// Get inner fs object
    pub fn inner(&self) -> &std::fs::ReadDir {
        self.standard.inner()
    }
    /// Get standard ReadDir object0
    pub fn standard(&self) -> &StandardReadDir {
        &self.standard
    }
    fn from_standard(standard: StandardReadDir) -> Self {
        Self {
            standard
        }
    }
}

/// Functions for FsReadDir
impl FsReadDir for UnixReadDir {
    type Context    = <UnixDirEntry as FsDirEntry>::Context;
    type Inner      = StandardReadDir;
    type Error      = std::io::Error;
    type DirEntry   = UnixDirEntry;

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.standard
    }

    fn process_inner_entry(&mut self, inner_entry: StandardDirEntry) -> Result<Self::DirEntry, Self::Error> {
        Self::DirEntry::from_standard(inner_entry)
    }
}

impl Iterator for UnixReadDir {
    type Item = Result<UnixDirEntry, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_fsentry(&mut ())
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Windows FsDirEntry implementation using std::fs::* objects 
#[derive(Debug)]
pub struct UnixDirEntry {
    standard: StandardDirEntry,

    /// The underlying inode number (Unix only).
    pub ino: u64,
}

impl UnixDirEntry {
    /// Get inner fs object
    pub fn inner(&self) -> &std::fs::DirEntry {
        self.standard.inner()
    }

    /// Get standard FsDirEntry implementation
    pub fn standard(&self) -> &StandardDirEntry {
        &self.standard
    }

    /// Makes optimized object from standard
    pub fn from_standard(standard: StandardDirEntry) -> Result<Self, std::io::Error> {
        use std::os::unix::fs::DirEntryExt;

        let ino = standard.inner().ino();
        Self {
            standard,
            ino,
        }.into_ok()
    }

    // fn file_name_from_path(
    //     path: &<Self as FsDirEntry>::Path,
    // ) -> <Self as FsDirEntry>::FileName {
    //     StandardDirEntry::file_name_from_path( path )
    // }

    // /// Get metadata
    // fn metadata_from_path(
    //     path: &<Self as FsDirEntry>::Path,
    //     follow_link: bool,
    //     _ctx: &mut <Self as FsDirEntry>::Context,
    // ) -> Result<<Self as FsDirEntry>::Metadata, <Self as FsDirEntry>::Error> {
    //     StandardDirEntry::metadata_from_path( path, follow_link )
    // }

    // /// Read dir
    // fn read_dir_from_path(
    //     path: &<Self as FsDirEntry>::Path,
    //     _ctx: &mut <Self as FsDirEntry>::Context,
    // ) -> Result<<Self as FsDirEntry>::ReadDir, <Self as FsDirEntry>::Error> {
    //     UnixReadDir {
    //         standard: StandardDirEntry::read_dir_from_path(path)?,
    //     }.into_ok()
    // }

    /// device_num
    fn device_num_from_path(
        path: &<Self as FsDirEntry>::Path,
    ) -> Result<<Self as FsDirEntry>::DeviceNum, <Self as FsDirEntry>::Error> {
        use std::os::unix::fs::MetadataExt;

        path.metadata().map(|md| md.dev())
    }
}

/// Functions for FsDirEntry
impl FsDirEntry for UnixDirEntry {
    type Context        = <StandardDirEntry as FsDirEntry>::Context;

    type Path           = <StandardDirEntry as FsDirEntry>::Path;
    type PathBuf        = <StandardDirEntry as FsDirEntry>::PathBuf;
    type FileName       = <StandardDirEntry as FsDirEntry>::FileName;

    type Error          = <StandardDirEntry as FsDirEntry>::Error;
    type FileType       = <StandardDirEntry as FsDirEntry>::FileType;
    type Metadata       = std::fs::Metadata;
    type ReadDir        = UnixReadDir;
    type DirFingerprint = <StandardDirEntry as FsDirEntry>::DirFingerprint;
    type DeviceNum      = u64;
    type RootDirEntry   = UnixRootDirEntry;

    /// Get path of this entry
    fn path(&self) -> &Self::Path {
        self.standard.path()
    }
    /// Get path of this entry
    fn pathbuf(&self) -> Self::PathBuf {
        self.standard.pathbuf()
    }
    /// Get path of this entry
    fn canonicalize(&self) -> Result<Self::PathBuf, Self::Error> {
        self.standard.canonicalize()
    }
    fn file_name(&self) -> Self::FileName {
        self.standard.file_name()
    }

    /// Get file type
    fn file_type(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<Self::FileType, Self::Error> {
        let metadata = self.metadata(follow_link, ctx)?;
        metadata.file_type().into_ok()
    }

    /// Get metadata
    fn metadata(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<Self::Metadata, Self::Error> {
        self.standard.metadata(follow_link, ctx)
    }

    /// Read dir
    fn read_dir(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::ReadDir, Self::Error> {
        UnixReadDir {
            standard: self.standard.read_dir(ctx)?,
        }.into_ok()
    }

    /// Return the unique handle
    fn fingerprint(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::DirFingerprint, Self::Error> {
        self.standard.fingerprint(ctx)
    }

    fn is_same(
        lhs: (&Self::Path, &Self::DirFingerprint),
        rhs: (&Self::Path, &Self::DirFingerprint),
    ) -> bool {
        StandardDirEntry::is_same( lhs, rhs )
    }

    /// device_num
    fn device_num(
        &self,
        _ctx: &mut Self::Context,
    ) -> Result<Self::DeviceNum, Self::Error> {
        Self::device_num_from_path( self.path() )
    }

    fn to_parts(
        &mut self,
        follow_link: bool,
        force_metadata: bool,
        force_file_name: bool,
        ctx: &mut Self::Context,
    ) -> (Self::PathBuf, Option<Self::Metadata>, Option<Self::FileName>) {
        self.standard.to_parts( follow_link, force_metadata, force_file_name, ctx )
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Windows FsRootDirEntry implementation using std::fs::* objects 
#[derive(Debug)]
pub struct UnixRootDirEntry {
    standard: StandardRootDirEntry,
}

/// Functions for FsDirEntry
impl FsRootDirEntry for UnixRootDirEntry {
    type Context    = <UnixDirEntry as FsDirEntry>::Context;
    type DirEntry   = UnixDirEntry;

    fn from_path(
        path: &<Self::DirEntry as FsDirEntry>::Path,
        ctx: &mut Self::Context,
    ) -> Result<Self, <Self::DirEntry as FsDirEntry>::Error> {
        Self {
            standard: StandardRootDirEntry::from_path( path, ctx )?,
        }.into_ok()
    }

    /// Get path of this entry
    fn path(&self) -> &<Self::DirEntry as FsDirEntry>::Path {
        self.standard.path()    
    }
    /// Get path of this entry
    fn pathbuf(&self) -> <Self::DirEntry as FsDirEntry>::PathBuf {
        self.standard.pathbuf()    
    }
    /// Get path of this entry
    fn canonicalize(&self) -> Result<<Self::DirEntry as FsDirEntry>::PathBuf, <Self::DirEntry as FsDirEntry>::Error> {
        self.standard.canonicalize()    
    }

    fn file_name(
        &self
    ) -> <Self::DirEntry as FsDirEntry>::FileName {
        self.standard.file_name()    
    }

    /// Get file type
    fn file_type(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::FileType, <Self::DirEntry as FsDirEntry>::Error> {
        self.standard.file_type( follow_link, ctx )
    }

    /// Get metadata
    fn metadata(
        &self,
        follow_link: bool,
        ctx: &mut <Self::DirEntry as FsDirEntry>::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::Metadata, <Self::DirEntry as FsDirEntry>::Error> {
        self.standard.metadata( follow_link, ctx )
    }

    /// Read dir
    fn read_dir(
        &self,
        ctx: &mut <Self::DirEntry as FsDirEntry>::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::ReadDir, <Self::DirEntry as FsDirEntry>::Error> {
        let rd = self.standard.read_dir( ctx )?;
        let readdir = UnixReadDir::from_standard(rd);
        readdir.into_ok()
    }

    /// Return the unique handle
    fn fingerprint(
        &self,
        ctx: &mut <Self::DirEntry as FsDirEntry>::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::DirFingerprint, <Self::DirEntry as FsDirEntry>::Error> {
        self.standard.fingerprint( ctx )
    }

    /// device_num
    fn device_num(
        &self,
        _ctx: &mut <Self::DirEntry as FsDirEntry>::Context,
    ) -> Result<<Self::DirEntry as FsDirEntry>::DeviceNum, <Self::DirEntry as FsDirEntry>::Error> {
        UnixDirEntry::device_num_from_path( self.path() )
    }

    fn to_parts(
        &mut self,
        follow_link: bool,
        force_metadata: bool,
        force_file_name: bool,
        ctx: &mut Self::Context,
    ) -> (<Self::DirEntry as FsDirEntry>::PathBuf, Option<<Self::DirEntry as FsDirEntry>::Metadata>, Option<<Self::DirEntry as FsDirEntry>::FileName>) {
        self.standard.to_parts( follow_link, force_metadata, force_file_name, ctx )
    }
}
