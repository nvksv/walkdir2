use crate::fs::standard::{StandardDirEntry, StandardReadDir, StandardRootDirEntry};
use crate::fs::{FsDirEntry, FsReadDir, FsRootDirEntry};
use crate::wd::IntoOk;

use std::fmt::Debug;
use std::fs;

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Windows FsReadDir implementation using std::fs::* objects 
#[derive(Debug)]
pub struct WindowsReadDir {
    standard: StandardReadDir,
}

impl WindowsReadDir {
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
impl FsReadDir for WindowsReadDir {
    type Context    = <WindowsDirEntry as FsDirEntry>::Context;
    type Inner      = StandardReadDir;
    type Error      = std::io::Error;
    type DirEntry   = WindowsDirEntry;

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.standard
    }

    fn process_inner_entry(&mut self, inner_entry: StandardDirEntry) -> Result<Self::DirEntry, Self::Error> {
        Self::DirEntry::from_standard(inner_entry)
    }
}

impl Iterator for WindowsReadDir {
    type Item = Result<WindowsDirEntry, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_fsentry(&mut ())
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Windows FsDirEntry implementation using std::fs::* objects 
#[derive(Debug)]
pub struct WindowsDirEntry {
    standard: StandardDirEntry,

    /// The underlying metadata (Windows only). We store this on Windows
    /// because this comes for free while reading a directory.
    ///
    /// We use this to determine whether an entry is a directory or not, which
    /// works around a bug in Rust's standard library:
    /// <https://github.com/rust-lang/rust/issues/46484>
    metadata: fs::Metadata,
}

impl WindowsDirEntry {
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
        let metadata = standard.inner().metadata()?;
        Self {
            metadata,
            standard,
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
    //     WindowsReadDir {
    //         standard: StandardDirEntry::read_dir_from_path(path)?,
    //     }.into_ok()
    // }

    /// device_num
    fn device_num_from_path(
        path: &<Self as FsDirEntry>::Path,
    ) -> Result<<Self as FsDirEntry>::DeviceNum, <Self as FsDirEntry>::Error> {
        use winapi_util::{file, Handle};

        let h = Handle::from_path_any(path)?;
        file::information(h).map(|info| info.volume_serial_number())
    }
}

/// Functions for FsDirEntry
impl FsDirEntry for WindowsDirEntry {
    type Context        = <StandardDirEntry as FsDirEntry>::Context;

    type Path           = <StandardDirEntry as FsDirEntry>::Path;
    type PathBuf        = <StandardDirEntry as FsDirEntry>::PathBuf;
    type FileName       = <StandardDirEntry as FsDirEntry>::FileName;

    type Error          = <StandardDirEntry as FsDirEntry>::Error;
    type FileType       = <StandardDirEntry as FsDirEntry>::FileType;
    type Metadata       = std::fs::Metadata;
    type ReadDir        = WindowsReadDir;
    type DirFingerprint = <StandardDirEntry as FsDirEntry>::DirFingerprint;
    type DeviceNum      = u64;
    type RootDirEntry   = WindowsRootDirEntry;

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
        if !follow_link {
            return self.metadata.file_type().into_ok();
        };

        let metadata = self.metadata(follow_link, ctx)?;
        metadata.file_type().into_ok()
    }

    /// Get metadata
    fn metadata(
        &self,
        follow_link: bool,
        ctx: &mut Self::Context,
    ) -> Result<Self::Metadata, Self::Error> {
        if !follow_link {
            return self.metadata.clone().into_ok();
        }; 
        
        self.standard.metadata(follow_link, ctx)
    }

    /// Read dir
    fn read_dir(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Self::ReadDir, Self::Error> {
        WindowsReadDir {
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
        let (fmd, md) = if !follow_link {
            (false, Some(self.metadata.clone()))
        } else {
            (force_metadata, None)
        };

        let (pathbuf, smd, n) = self.standard.to_parts( follow_link, fmd, force_file_name, ctx );

        let md = if !follow_link {
            md
        } else {
            smd
        };

        (pathbuf, md, n)
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////

/// An optimized for Windows FsRootDirEntry implementation using std::fs::* objects 
#[derive(Debug)]
pub struct WindowsRootDirEntry {
    standard: StandardRootDirEntry,
}

/// Functions for FsDirEntry
impl FsRootDirEntry for WindowsRootDirEntry {
    type Context    = <WindowsDirEntry as FsDirEntry>::Context;
    type DirEntry   = WindowsDirEntry;

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
        let readdir = WindowsReadDir::from_standard(rd);
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
        WindowsDirEntry::device_num_from_path( self.path() )
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
