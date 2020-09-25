use crate::fs;

// use crate::cp::ContentProcessor;
// pub use crate::dent::DirEntry;
use crate::error::{Error, ErrorInner};

/// An useful wrapper for Some(...) ready to chaining
pub trait IntoSome<T> {
    /// Some(...)
    fn into_some(self) -> Option<T>;
}

impl<T> IntoSome<T> for T {
    fn into_some(self) -> Option<Self> {
        Some(self)
    }
}

/// An useful wrapper for Ok(...) ready to chaining
pub trait IntoOk<T, E> {
    /// Ok(...)
    fn into_ok(self) -> std::result::Result<T, E>;
}

impl<T, E> IntoOk<T, E> for T {
    fn into_ok(self) -> std::result::Result<Self, E> {
        Ok(self)
    }
}

/// An useful wrapper for Err(...) ready to chaining
pub trait IntoErr<T, E> {
    /// Err(...)
    fn into_err(self) -> std::result::Result<T, E>;
}

impl<T, E> IntoErr<T, E> for E {
    fn into_err(self) -> std::result::Result<T, Self> {
        Err(self)
    }
}

/// Type of depth
pub type Depth = usize;

/// A result type for walkdir operations.
///
/// Note that this result type embeds the error type in this crate. This
/// is only useful if you care about the additional information provided by
/// the error (such as the path associated with the error or whether a loop
/// was dectected). If you want things to Just Work, then you can use
/// [`io::Result`] instead since the error type in this package will
/// automatically convert to an [`io::Result`] when using the [`try!`] macro.
///
/// [`io::Result`]: https://doc.rust-lang.org/stable/std/io/type.Result.html
/// [`try!`]: https://doc.rust-lang.org/stable/std/macro.try.html
pub type Result<T, E> = ::std::result::Result<T, Error<E>>;

/// A result type for walkdir operations with inner errors.
pub type ResultInner<T, E> =
    ::std::result::Result<T, ErrorInner<E>>;

/// A DirEntry sorter function.
pub type FnCmp<E> = Box<
    dyn FnMut( (&E, &<E as fs::FsDirEntry>::FileType), (&E, &<E as fs::FsDirEntry>::FileType), &mut <E as fs::FsDirEntry>::Context, ) -> std::cmp::Ordering
        + Send
        + Sync
        + 'static,
>;

// Convert FsReadDir.next() to some Option<T>.
// - Some(T) -- add T to collected vec,
// - None -- entry must be ignored
//pub trait FnProcessFsDirEntry<E: storage::StorageExt, T>: FnMut(self::Result<DirEntry<E>, E>) -> Option<T> {}

/// Follow symlinks and check same_file_system. Also determine is_dir flag.
/// - Some(Ok((dent, is_dir))) -- normal entry to yielding
/// - Some(Err(_)) -- some error occured
/// - None -- entry must be ignored
//pub type ProcessDirEntry<E: storage::StorageExt> = self::Result<(DirEntry<E>, bool), E>

/// A variants for filtering content
#[derive(Debug, PartialEq, Eq)]
pub enum ContentFilter {
    /// No filter, all content will be yielded (default)
    None,
    /// Yield files only (not dirs, i.e. including symlinks)
    FilesOnly,
    /// Yield dirs only
    DirsOnly,
    /// Skip all (only BeforeContent(dent) and AfterContent will be yielded)
    SkipAll,
}

/// A variants for ordering content
#[derive(Debug, PartialEq, Eq)]
pub enum ContentOrder {
    /// No arrange (default)
    None,
    /// Yield files first, then dirs
    FilesFirst,
    /// Yield dirs (with theirs content) first, then files
    DirsFirst,
}

/// A position in dirs tree
#[derive(Debug, PartialEq, Eq)]
pub enum Position<BC, EN, ER> {
    /// Before content of current dir
    BeforeContent(BC),
    /// An entry
    Entry(EN),
    /// An error
    Error(ER),
    /// After content of current dir
    AfterContent,
}


