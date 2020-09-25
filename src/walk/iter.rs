use crate::walk::classic_iter::ClassicIter;
use crate::cp::ContentProcessor;
use crate::fs;
use crate::walk::walk::{WalkDirIterator, WalkDirIteratorItem};
use crate::wd::{Position};

/////////////////////////////////////////////////////////////////////////
//// WalkDirIter

/// WalkDirIter
pub trait WalkDirIter<E, CP>: Sized + Iterator<Item = WalkDirIteratorItem<E, CP>>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// Yields only entries which satisfy the given predicate and skips
    /// descending into directories that do not satisfy the given predicate.
    ///
    /// The predicate is applied to all entries. If the predicate is
    /// true, iteration carries on as normal. If the predicate is false, the
    /// entry is ignored and if it is a directory, it is not descended into.
    ///
    /// This is often more convenient to use than [`skip_current_dir`]. For
    /// example, to skip hidden files and directories efficiently on unix
    /// systems:
    ///
    /// ```no_run
    /// use walkdir2::{DirEntry, WalkDir, WalkDirIter, ClassicWalkDirIter};
    /// # use walkdir2::Error;
    ///
    /// fn is_hidden(entry: &DirEntry) -> bool {
    ///     entry.file_name()
    ///          .to_str()
    ///          .map(|s| s.starts_with("."))
    ///          .unwrap_or(false)
    /// }
    ///
    /// # fn try_main() -> Result<(), Error> {
    /// for entry in WalkDir::new("foo")
    ///                      .into_classic()
    ///                      .filter_entry(|e| !is_hidden(e)) {
    ///     println!("{}", entry?.path().display());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Note that the iterator will still yield errors for reading entries that
    /// may not satisfy the predicate.
    ///
    /// Note that entries skipped with [`min_depth`] and [`max_depth`] are not
    /// passed to this predicate.
    ///
    /// Note that if the iterator has `contents_first` enabled, then this
    /// method is no different than calling the standard `Iterator::filter`
    /// method (because directory entries are yielded after they've been
    /// descended into).
    ///
    /// [`skip_current_dir`]: #method.skip_current_dir
    /// [`min_depth`]: struct.WalkDir.html#method.min_depth
    /// [`max_depth`]: struct.WalkDir.html#method.max_depth
    fn filter_entry<P>(self, predicate: P) -> FilterEntry<E, CP, Self, P>
    where
        P: FnMut(&CP::Item) -> bool,
    {
        FilterEntry { inner: self, predicate, _cp: std::marker::PhantomData }
    }

    /// WalkDirIter
    fn skip_current_dir(&mut self);

    /// WalkDirIter
    fn into_classic(self) -> ClassicIter<E, CP, Self> {
        ClassicIter::<E, CP, Self>::new(self)
    }
}

impl<E, CP> WalkDirIter<E, CP> for WalkDirIterator<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    fn skip_current_dir(&mut self) {
        WalkDirIterator::<E, CP>::skip_current_dir(self);
    }
}

/////////////////////////////////////////////////////////////////////////
//// FilterEntry

/// A recursive directory iterator that skips entries.
///
/// Values of this type are created by calling [`.filter_entry()`] on an
/// `IntoIter`, which is formed by calling [`.into_iter()`] on a `WalkDir`.
///
/// Directories that fail the predicate `P` are skipped. Namely, they are
/// never yielded and never descended into.
///
/// Entries that are skipped with the [`min_depth`] and [`max_depth`] options
/// are not passed through this filter.
///
/// If opening a handle to a directory resulted in an error, then it is yielded
/// and no corresponding call to the predicate is made.
///
/// Type parameter `I` refers to the underlying iterator and `P` refers to the
/// predicate, which is usually `FnMut(&DirEntry) -> bool`.
///
/// [`.filter_entry()`]: struct.IntoIter.html#method.filter_entry
/// [`.into_iter()`]: struct.WalkDir.html#into_iter.v
/// [`min_depth`]: struct.WalkDir.html#method.min_depth
/// [`max_depth`]: struct.WalkDir.html#method.max_depth
#[derive(Debug)]
pub struct FilterEntry<E, CP, I, P>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
    I: Iterator<Item = WalkDirIteratorItem<E, CP>> + WalkDirIter<E, CP>,
    P: FnMut(&CP::Item) -> bool,
{
    inner: I,
    predicate: P,
    _cp: std::marker::PhantomData<CP>,
}

impl<E, CP, I, P> Iterator for FilterEntry<E, CP, I, P>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
    I: Iterator<Item = WalkDirIteratorItem<E, CP>> + WalkDirIter<E, CP>,
    P: FnMut(&CP::Item) -> bool,
{
    type Item = WalkDirIteratorItem<E, CP>;

    /// Advances the iterator and returns the next value.
    ///
    /// # Errors
    ///
    /// If the iterator fails to retrieve the next value, this method returns
    /// an error value. The error will be wrapped in an `Option::Some`.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = match self.inner.next() {
                Some(item) => item,
                None => return None,
            };

            if let Position::Entry(ref dent) = item {
                if !(self.predicate)(dent) {
                    if CP::is_dir(dent) {
                        self.inner.skip_current_dir();
                    }
                    continue;
                }
            }

            return Some(item);
        }
    }
}

impl<E, CP, I, P> FilterEntry<E, CP, I, P>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
    I: Iterator<Item = WalkDirIteratorItem<E, CP>> + WalkDirIter<E, CP>,
    P: FnMut(&CP::Item) -> bool,
{
    /// Yields only entries which satisfy the given predicate and skips
    /// descending into directories that do not satisfy the given predicate.
    ///
    /// The predicate is applied to all entries. If the predicate is
    /// true, iteration carries on as normal. If the predicate is false, the
    /// entry is ignored and if it is a directory, it is not descended into.
    ///
    /// This is often more convenient to use than [`skip_current_dir`]. For
    /// example, to skip hidden files and directories efficiently on unix
    /// systems:
    ///
    /// ```no_run
    /// use walkdir2::{DirEntry, WalkDir, WalkDirIter, ClassicWalkDirIter};
    /// # use walkdir2::Error;
    ///
    /// fn is_hidden(entry: &DirEntry) -> bool {
    ///     entry.file_name()
    ///          .to_str()
    ///          .map(|s| s.starts_with("."))
    ///          .unwrap_or(false)
    /// }
    ///
    /// # fn try_main() -> Result<(), Error> {
    /// for entry in WalkDir::new("foo")
    ///                      .into_classic()
    ///                      .filter_entry(|e| !is_hidden(e)) {
    ///     println!("{}", entry?.path().display());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Note that the iterator will still yield errors for reading entries that
    /// may not satisfy the predicate.
    ///
    /// Note that entries skipped with [`min_depth`] and [`max_depth`] are not
    /// passed to this predicate.
    ///
    /// Note that if the iterator has `contents_first` enabled, then this
    /// method is no different than calling the standard `Iterator::filter`
    /// method (because directory entries are yielded after they've been
    /// descended into).
    ///
    /// [`skip_current_dir`]: #method.skip_current_dir
    /// [`min_depth`]: struct.WalkDir.html#method.min_depth
    /// [`max_depth`]: struct.WalkDir.html#method.max_depth
    pub fn filter_entry(self, predicate: P) -> FilterEntry<E, CP, Self, P> {
        FilterEntry { inner: self, predicate, _cp: std::marker::PhantomData }
    }

    /// Skips the current directory.
    ///
    /// This causes the iterator to stop traversing the contents of the least
    /// recently yielded directory. This means any remaining entries in that
    /// directory will be skipped (including sub-directories).
    ///
    /// Note that the ergonomics of this method are questionable since it
    /// borrows the iterator mutably. Namely, you must write out the looping
    /// condition manually. For example, to skip hidden entries efficiently on
    /// unix systems:
    ///
    /// ```no_run
    /// use walkdir2::{DirEntry, WalkDir, WalkDirIter, ClassicWalkDirIter};
    ///
    /// fn is_hidden(entry: &DirEntry) -> bool {
    ///     entry.file_name()
    ///          .to_str()
    ///          .map(|s| s.starts_with("."))
    ///          .unwrap_or(false)
    /// }
    ///
    /// let mut it = WalkDir::new("foo").into_classic();
    /// loop {
    ///     let entry = match it.next() {
    ///         None => break,
    ///         Some(Err(err)) => panic!("ERROR: {}", err),
    ///         Some(Ok(entry)) => entry,
    ///     };
    ///     if is_hidden(&entry) {
    ///         if entry.file_type().is_dir() {
    ///             it.skip_current_dir();
    ///         }
    ///         continue;
    ///     }
    ///     println!("{}", entry.path().display());
    /// }
    /// ```
    ///
    /// You may find it more convenient to use the [`filter_entry`] iterator
    /// adapter. (See its documentation for the same example functionality as
    /// above.)
    ///
    /// [`filter_entry`]: #method.filter_entry
    pub fn skip_current_dir(&mut self) {
        self.inner.skip_current_dir();
    }
}

impl<E, CP, I, P> WalkDirIter<E, CP> for FilterEntry<E, CP, I, P>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
    I: Iterator<Item = WalkDirIteratorItem<E, CP>> + WalkDirIter<E, CP>,
    P: FnMut(&CP::Item) -> bool,
{
    fn skip_current_dir(&mut self) {
        self.inner.skip_current_dir();
    }
}
