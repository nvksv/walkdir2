use std::vec;

use crate::cp::ContentProcessor;
use crate::fs::{self, FsFileType};
use crate::walk::dir::{DirState, FlatDirEntry};
use crate::walk::rawdent::{RawDirEntry};
use crate::error::{ErrorInner, Error};
use crate::walk::opts::{WalkDirOptions, WalkDirOptionsImmut};
use crate::wd::{
    self, ContentFilter, Depth, FnCmp, IntoOk, IntoSome, Position, InnerPositionWithData,
};

// /// Like try, but for iterators that return [`Option<Result<_, _>>`].
// ///
// /// [`Option<Result<_, _>>`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html
// macro_rules! ortry {
//     ($e:expr) => {
//         match $e {
//             Ok(v) => v,
//             Err(err) => return Some(Err(From::from(err))),
//         }
//     };
// }

// /// Like try, but for iterators that return [`Option<Result<_, _>>`].
// ///
// /// [`Option<Result<_, _>>`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html
// macro_rules! rtry {
//     ($e:expr) => {
//         match $e {
//             Ok(v) => v,
//             Err(err) => return Err(From::from(err)),
//         }
//     };
// }

macro_rules! debug {
    ($($arg:tt)*) => (if cfg!(debug_assertions) { $($arg)* })
}

macro_rules! process_dent {
    ($self:expr, $depth:expr) => {
        process_dent!(&$self.opts.immut, &$self.root_device, &$self.ancestors, $depth)
    };
    ($opts_immut:expr, $root_device:expr, $ancestors:expr, $depth:expr) => {
        (|opts_immut, root_device, ancestors, depth| {
            move |raw_dent: RawDirEntry<E>, ctx: &mut E::Context| {
                Self::process_rawdent(raw_dent, depth, opts_immut, root_device, ancestors, ctx)
            }
        })($opts_immut, $root_device, $ancestors, $depth)
    };
}

/// Type of item for Iterators
pub type WalkDirIteratorItem<E, CP> = Position<
    <CP as ContentProcessor<E>>::Item,
    <CP as ContentProcessor<E>>::Collection,
    Error<E>,
>;

/////////////////////////////////////////////////////////////////////////
//// Ancestor

/// An ancestor is an item in the directory tree traversed by walkdir, and is
/// used to check for loops in the tree when traversing symlinks.
#[derive(Debug)]
struct Ancestor<E: fs::FsDirEntry> {
    /// The path of this ancestor.
    path: E::PathBuf,
    /// Fingerprint
    fingerprint: E::DirFingerprint,
}

impl<E: fs::FsDirEntry> Ancestor<E> {
    /// Create a new ancestor from the given directory path.
    pub fn new(
        raw: &RawDirEntry<E>,
        ctx: &mut E::Context,
    ) -> wd::ResultInner<Self, E> {
        Self { 
            path: raw.pathbuf(), 
            fingerprint: raw.fingerprint(ctx)? 
        }.into_ok()
    }

    /// Returns true if and only if the given open file handle corresponds to
    /// the same directory as this ancestor.
    fn is_same(&self, rhs: &Self) -> bool {
        E::is_same( (&self.path, &self.fingerprint), (&rhs.path, &rhs.fingerprint))
    }
}

/////////////////////////////////////////////////////////////////////////
//// IntoIter

#[derive(Debug, PartialEq, Eq)]
enum TransitionState {
    None,
    CloseOldestBeforePushDown,
    BeforePushDown,
    BeforePopUp,
    AfterPopUp,
}

/// An iterator for recursively descending into a directory.
///
/// A value with this type must be constructed with the [`WalkDir`] type, which
/// uses a builder pattern to set options such as min/max depth, max open file
/// descriptors and whether the iterator should follow symbolic links. After
/// constructing a `WalkDir`, call [`.into_iter()`] at the end of the chain.
///
/// The order of elements yielded by this iterator is unspecified.
///
/// [`WalkDir`]: struct.WalkDir.html
/// [`.into_iter()`]: struct.WalkDir.html#into_iter.v
#[derive(Debug)]
pub struct WalkDirIterator<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// Options specified in the builder. Depths, max fds, etc.
    opts: WalkDirOptions<E, CP>,
    /// The start path.
    ///
    /// This is only `Some(...)` at the beginning. After the first iteration,
    /// this is always `None`.
    root: Option<E::PathBuf>,
    /// A stack of open (up to max fd) or closed handles to directories.
    /// An open handle is a plain [`fs::ReadDir`] while a closed handle is
    /// a `Vec<fs::DirEntry>` corresponding to the as-of-yet consumed entries.
    ///
    /// [`fs::ReadDir`]: https://doc.rust-lang.org/stable/std/fs/struct.ReadDir.html
    states: Vec<DirState<E, CP>>,
    /// before push down / after pop up
    transition_state: TransitionState,
    /// A stack of file paths.
    ///
    /// This is *only* used when [`follow_links`] is enabled. In all other
    /// cases this stack is empty.
    ///
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    ancestors: Vec<Ancestor<E>>,
    /// Count of opened dirs.
    opened_count: Depth,
    /// The current depth of iteration (the length of the stack at the
    /// beginning of each iteration).
    depth: Depth,
    /// The device of the root file path when the first call to `next` was
    /// made.
    ///
    /// If the `same_file_system` option isn't enabled, then this is always
    /// `None`. Conversely, if it is enabled, this is always `Some(...)` after
    /// handling the root path.
    root_device: Option<E::DeviceNum>,
}

type PushDirData<E, CP> = (DirState<E, CP>, Option<Ancestor<E>>);

impl<E, CP> WalkDirIterator<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// Make new
    pub fn new(opts: WalkDirOptions<E, CP>, root: E::PathBuf) -> Self {
        Self {
            opts,
            root: Some(root),
            states: vec![],
            transition_state: TransitionState::None,
            ancestors: vec![],
            opened_count: 0,
            depth: 0,
            root_device: None,
        }
    }

    #[cfg(debug_assertions)]
    fn do_debug_checks(&self) {
        
        // Check opened_count
        let mut real_count: Depth = 0;
        for state in &self.states {
            if state.is_open() {
                real_count += 1;
            }
        }

        assert_eq!( self.opened_count, real_count );

    }

    // Follow symlinks and check same_file_system. Also determine is_dir flag.
    // - Some(Ok((dent, is_dir))) -- normal entry to yielding
    // - Some(Err(_)) -- some error occured
    // - None -- entry must be ignored
    fn process_rawdent(
        rawdent: RawDirEntry<E>,
        depth: Depth,
        opts_immut: &WalkDirOptionsImmut,
        root_device_opt: &Option<E::DeviceNum>,
        ancestors: &Vec<Ancestor<E>>,
        ctx: &mut E::Context,
    ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>> {
        let (rawdent, loop_link) =
            if rawdent.is_symlink() && opts_immut.follow_links {
                let (rawdent, loop_link) = match Self::follow(rawdent, ancestors, ctx) {
                    Ok(v) => v,
                    Err(err) => return Err(err).into_some(),    
                };
                (rawdent, loop_link)
            } else {
                (rawdent, None)
            };

        let mut is_normal_dir = !rawdent.is_symlink() && rawdent.is_dir();

        if is_normal_dir {
            if opts_immut.same_file_system && depth > 0 {
                let root_device = root_device_opt.as_ref().expect("BUG: called is_same_file_system without root device");
                match Self::is_same_file_system(root_device, &rawdent, ctx) {
                    Ok(true) => {},
                    Ok(false) => return None,
                    Err(err) => return Err(err).into_some(),    
                }
            };
        } else if depth == 0 && rawdent.is_symlink() {
            // As a special case, if we are processing a root entry, then we
            // always follow it even if it's a symlink and follow_links is
            // false. We are careful to not let this change the semantics of
            // the DirEntry however. Namely, the DirEntry should still respect
            // the follow_links setting. When it's disabled, it should report
            // itself as a symlink. When it's enabled, it should always report
            // itself as the target.
            is_normal_dir = match rawdent.file_type_follow(ctx) {
                Ok(v) => v,
                Err(err) => return Err(err).into_some(),    
            }.is_dir();
        };

        FlatDirEntry { 
            raw: rawdent, 
            is_dir: is_normal_dir, 
            loop_link 
        }.into_ok().into_some()
    }

    fn init(
        &mut self, 
        root_path: &E::Path, 
    ) -> wd::ResultInner<(), E> {
        let root = RawDirEntry::<E>::from_path( root_path, &mut self.opts.ctx )?;

        if self.opts.immut.same_file_system {
            self.root_device = Some(root.device_num(&mut self.opts.ctx)?);
        }

        self.push_root(root, 0)?;

        Ok(())
    }

    fn push_root(
        &mut self, 
        root: RawDirEntry<E>, 
        depth: Depth
    ) -> wd::ResultInner<(), E> {
        let state = DirState::<E, CP>::new_once(
            root,
            depth,
            &self.opts.immut,
            &mut self.opts.sorter,
            &mut process_dent!(self, depth),
            &mut self.opened_count,
            &mut self.opts.ctx,
        )?;

        self.push_dir_2( (state, None) );

        Ok(())
    }

    fn check_max_open(&mut self) {
        // Exit when open handles count are not limited.
        let max_open = if let Some(max_open) = self.opts.immut.max_open {max_open} else {return};

        // Exit if current open handles count are under limit
        if self.opened_count < max_open {return};
        // opened_count cannot be greater then max_open
        assert!(self.opened_count == max_open);

        // Because of max_open >= 1
        assert!(self.opened_count > 0);

        // Search for high open
        for state in self.states.iter_mut() {
            if state.is_open() {
                let was_open = state.load_all(
                    &self.opts.immut,
                    &mut process_dent!(self, state.depth()),
                    &mut self.opened_count,
                    &mut self.opts.ctx,
                );
                debug_assert!(was_open);
                return;
            }
        }

        unreachable!()
    }

    fn push_dir_1(
        flat: &FlatDirEntry<E>,
        new_depth: Depth,
        opts_immut: &WalkDirOptionsImmut,
        sorter: &mut Option<FnCmp<E>>,
        root_device: &Option<E::DeviceNum>,
        ancestors: &Vec<Ancestor<E>>,
        opened_count: &mut Depth,
        ctx: &mut E::Context,
    ) -> wd::ResultInner<PushDirData<E, CP>, E> {
        // This is safe as we makes any changes strictly AFTER using dent_ptr.
        // Neither E::read_dir nor Ancestor::new

        assert!(flat.loop_link.is_none());

        // Open a handle to reading the directory's entries.
        let state = DirState::<E, CP>::new(
            &flat.raw,
            new_depth,
            opts_immut,
            sorter,
            &mut process_dent!(opts_immut, root_device, ancestors, new_depth),
            opened_count,
            ctx,
        )?;

        let ancestor = if opts_immut.follow_links {
            let ancestor = Ancestor::new(&flat.raw, ctx)?;
            Some(ancestor)
        } else {
            None
        };

        // // If we had to close out a previous directory stream, then we need to
        // // increment our index the oldest still-open stream. We do this only
        // // after adding to our stack, in order to ensure that the oldest_opened
        // // index remains valid. The worst that can happen is that an already
        // // closed stream will be closed again, which is a no-op.
        // //
        // // We could move the close of the stream above into this if-body, but
        // // then we would have more than the maximum number of file descriptors
        // // open at a particular point in time.
        // if free == self.opts.immut.max_open {
        //     // Unwrap is safe here because self.oldest_opened is guaranteed to
        //     // never be greater than `self.stack_list.len()`, which implies
        //     // that the subtraction won't underflow and that adding 1 will
        //     // never overflow.
        //     self.oldest_opened = self.oldest_opened.checked_add(1).unwrap();
        // };

        Ok((state, ancestor))
    }

    fn push_dir_2(&mut self, data: PushDirData<E, CP>) {
        let (state, ancestor_opt) = data;

        if let Some(ancestor) = ancestor_opt {
            self.ancestors.push(ancestor);
        }

        // if self.opts.immut.max_open.is_some() && state.is_open() {
        //     self.opened_count += 1;
        // };

        self.states.push(state);
    }

    fn pop_dir(&mut self) {

        let last_state = self.states.pop().expect("BUG: cannot pop from empty stack");
        last_state.on_drop(&mut self.opened_count);

        if self.opts.immut.follow_links {
            self.ancestors.pop().expect("BUG: list/path stacks out of sync");
        }

        debug!(self.do_debug_checks());
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
        if let Some(cur_state) = self.states.last_mut() {
            cur_state.skip_all();
            self.transition_state = TransitionState::None;
        }
    }

    fn follow(
        raw: RawDirEntry<E>,
        ancestors: &Vec<Ancestor<E>>,
        ctx: &mut E::Context,
    ) -> wd::ResultInner<(RawDirEntry<E>, Option<Depth>), E> {
        let dent = raw.follow(ctx)?;

        let loop_link = if dent.is_dir() && !ancestors.is_empty() {
            Self::check_loop( &dent, ancestors, ctx )?
        } else {
            None
        };

        Ok((dent, loop_link))
    }

    fn check_loop(
        raw: &RawDirEntry<E>,
        ancestors: &Vec<Ancestor<E>>,
        ctx: &mut E::Context,
    ) -> wd::ResultInner<Option<Depth>, E> {
        let raw_as_ancestor = Ancestor::<E>::new( raw, ctx )?;

        for (index, ancestor) in ancestors.iter().enumerate().rev() {
            if ancestor.is_same(&raw_as_ancestor) {
                return Ok(Some(index));
            }
        }

        Ok(None)
    }

    fn make_loop_error(
        ancestors: &Vec<Ancestor<E>>,
        depth: Depth,
        child: &E::Path,
    ) -> ErrorInner<E> {
        let ancestor = ancestors.get(depth).unwrap();

        ErrorInner::<E>::from_loop(&ancestor.path, child)
    }

    fn is_same_file_system(
        root_device: &E::DeviceNum,
        dent: &RawDirEntry<E>,
        ctx: &mut E::Context,
    ) -> wd::ResultInner<bool, E> {
        Ok(*root_device == dent.device_num(ctx)?)
    }

    /// Gets content of current dir
    pub fn get_current_dir_content(&mut self, filter: ContentFilter) -> CP::Collection {
        let cur_state = self.states.last_mut().unwrap();

        let content = cur_state.clone_all_content(
            filter,
            &self.opts.immut,
            &mut self.opts.content_processor,
            &mut process_dent!(self, cur_state.depth()),
            &mut self.opened_count,
            &mut self.opts.ctx,
        );

        content
    }
}

macro_rules! next_and_yield_rflat {
    ($self:expr, $cur_state:expr, $cur_depth:expr, $rflat:expr) => {{
        let odent = $rflat.make_content_item(&mut $self.opts.content_processor, &mut $self.opts.ctx);
        $cur_state.next_position(
            &$self.opts.immut,
            &mut process_dent!($self, $cur_depth),
            &mut $self.opened_count,
            &mut $self.opts.ctx,
        );
        if let Some(dent) = odent {
            return Position::Entry(dent).into_some();
        } else {
            false
        }
    }};
}

macro_rules! yield_rflat {
    ($self:expr, $cur_state:expr, $cur_depth:expr, $rflat:expr) => {{
        let odent = $rflat.make_content_item(&mut $self.opts.content_processor, &mut $self.opts.ctx);
        if let Some(dent) = odent {
            return Position::Entry(dent).into_some();
        } else {
            false
        }
    }};
}

impl<E, CP> Iterator for WalkDirIterator<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    type Item = WalkDirIteratorItem<E, CP>;
    /// Advances the iterator and returns the next value.
    ///
    /// # Errors
    ///
    /// If the iterator fails to retrieve the next value, this method returns
    /// an error value. The error will be wrapped in an Option::Some.
    fn next(&mut self) -> Option<Self::Item> {
        fn get_parent_dent<E, CP>(this: &mut WalkDirIterator<E, CP>, cur_depth: Depth) -> CP::Item
        where
            E: fs::FsDirEntry,
            CP: ContentProcessor<E>,
        {
            let prev_state = this.states.get_mut(cur_depth - 1).unwrap();
            match prev_state.get_current_position() {
                InnerPositionWithData::Entry(mut rflat) => {
                    rflat.make_content_item(&mut this.opts.content_processor, &mut this.opts.ctx).unwrap()
                }
                _ => unreachable!(),
            }
        }

        // Initial actions
        if let Some(root_path) = self.root.take() {
            if let Err(e) = self.init(&root_path) {
                debug!(self.do_debug_checks());
                return Position::Error(Error::from_inner(e, 0)).into_some();
                // Here self.states is empty, so next call will always return None.
            };
        }

        loop {
            let cur_depth = match self.states.len() {
                0 => unreachable!(),
                len @ _ => (len - 1),
            };

            // Close one opened handle
            if self.transition_state == TransitionState::CloseOldestBeforePushDown {
                self.check_max_open();
                self.transition_state = TransitionState::BeforePushDown;
                continue;
            }

            let cur_state = self.states.get_mut(cur_depth).unwrap();

            match cur_state.get_current_position() {
                // Before content
                InnerPositionWithData::OpenDir => {
                    // Before content of current dir
                    assert!(self.transition_state == TransitionState::None);

                    // Shift to first entry
                    cur_state.next_position(
                        &self.opts.immut,
                        &mut process_dent!(self, cur_depth),
                        &mut self.opened_count,
                        &mut self.opts.ctx,
                    );

                    // At root we dont't yield Position::OpenDir (OpenDirWithContent)
                    if cur_depth == 0 {
                        continue;
                    }

                    if self.opts.immut.yield_open_dir_with_content {
                        let content = cur_state.clone_all_content(
                            self.opts.immut.open_dir_with_content_filter,
                            &self.opts.immut,
                            &mut self.opts.content_processor,
                            &mut process_dent!(self, cur_state.depth()),
                            &mut self.opened_count,
                            &mut self.opts.ctx,
                        );
                        let parent = get_parent_dent(self, cur_depth);
                        debug!(self.do_debug_checks());
                        return Position::OpenDirWithContent(parent, content).into_some();
                    } else {
                        let parent = get_parent_dent(self, cur_depth);
                        debug!(self.do_debug_checks());
                        return Position::OpenDir(parent).into_some();
                    }
                },
                // At entry
                InnerPositionWithData::Entry(mut rflat) => {
                    // Process entry

                    // Allow yield this entry if (require all):
                    // - It isn't hidden
                    // - Current depth is in allowed range
                    // - Allowed to yield loop links (for loop links)
                    let allow_yield = !rflat.hidden()
                        && (cur_depth >= self.opts.immut.min_depth)
                        && (if rflat.loop_link().is_some() {
                            self.opts.immut.yield_loop_links
                        } else {
                            true
                        });

                    if rflat.is_dir() {
                        // Process dir entry

                        match self.transition_state {
                            // First step
                            TransitionState::None => {
                                // If (cur_depth + 1) still in allowed range ...
                                let allow_push = cur_depth < self.opts.immut.max_depth && rflat.allow_push(&self.opts.content_processor);

                                if allow_push {
                                    // Check if rflat is loop link
                                    if let Some(loop_depth) = rflat.loop_link() {
                                        // Skip all children and jump to last step
                                        self.transition_state = TransitionState::AfterPopUp;

                                        // If yielding loop links not allowed, yield loop error
                                        if !self.opts.immut.yield_loop_links {
                                            let err = Self::make_loop_error(
                                                &self.ancestors,
                                                loop_depth,
                                                rflat.path(),
                                            );
                                            debug!(self.do_debug_checks());
                                            return Position::Error(Error::from_inner(
                                                err, cur_depth,
                                            ))
                                            .into_some();
                                        }
                                        continue;
                                    }

                                    // Before open new dir, we must close opened one
                                    self.transition_state =
                                        TransitionState::CloseOldestBeforePushDown;
                                } else {
                                    // Skip all children and jump to last step
                                    self.transition_state = TransitionState::AfterPopUp;
                                }

                                // In content_first mode: yield Position::Entry (if allowed) and shift to next entry
                                if !self.opts.immut.contents_first && allow_yield {
                                    if !yield_rflat!(self, cur_state, cur_depth, rflat) {
                                        // If conversion to CP::Item failed, skip all children and jump to last step
                                        self.transition_state = TransitionState::AfterPopUp;
                                    }
                                };
                            }
                            // Third step: now we might to open handle, so we need to go deeper!
                            TransitionState::BeforePushDown => {
                                // Deeper dir must start with empty state
                                self.transition_state = TransitionState::None;

                                match Self::push_dir_1(
                                    rflat.as_flat(),
                                    cur_depth + 1,
                                    &self.opts.immut,
                                    &mut self.opts.sorter,
                                    &self.root_device,
                                    &self.ancestors,
                                    &mut self.opened_count,
                                    &mut self.opts.ctx,
                                ) {
                                    Ok(data) => {
                                        self.push_dir_2(data);
                                    }
                                    Err(err) => {
                                        // Jump to last step
                                        self.transition_state = TransitionState::AfterPopUp;
                                        // And yield an error
                                        debug!(self.do_debug_checks());
                                        return Position::Error(Error::from_inner(
                                            err, cur_depth,
                                        ))
                                        .into_some();
                                    }
                                }
                            }
                            // Last step: here we processed all rflat's children (or skipped them all)
                            TransitionState::AfterPopUp => {
                                // Clear state
                                self.transition_state = TransitionState::None;

                                // In !content_first mode: yield Position::Entry (if allowed) and shift to next entry
                                if self.opts.immut.contents_first && allow_yield {
                                    next_and_yield_rflat!(self, cur_state, cur_depth, rflat);
                                // If conversion to CP::Item failed, ignore it
                                } else {
                                    cur_state.next_position(
                                        &self.opts.immut,
                                        &mut process_dent!(self, cur_depth),
                                        &mut self.opened_count,
                                        &mut self.opts.ctx,
                                    );
                                };
                            }
                            _ => unreachable!(),
                        };
                    } else {
                        // Process non-dir entry
                        assert!(self.transition_state == TransitionState::None);

                        // Yield Position::Entry (if allowed) and shift to next entry
                        if allow_yield {
                            next_and_yield_rflat!(self, cur_state, cur_depth, rflat);
                        // If conversion to CP::Item failed, ignore it
                        } else {
                            cur_state.next_position(
                                &self.opts.immut,
                                &mut process_dent!(self, cur_depth),
                                &mut self.opened_count,
                                &mut self.opts.ctx,
                            );
                        };
                    }
                }
                // At error
                InnerPositionWithData::Error(rerr) => {
                    // Process error
                    assert!(self.transition_state == TransitionState::None);

                    // Yield Position::Error and shift to next entry
                    let err = rerr.into_error();
                    cur_state.next_position(
                        &self.opts.immut,
                        &mut process_dent!(self, cur_depth),
                        &mut self.opened_count,
                        &mut self.opts.ctx,
                    );
                    debug!(self.do_debug_checks());
                    return Position::Error(err).into_some();
                }
                // After content of dir
                InnerPositionWithData::CloseDir => {
                    // After content of current dir

                    // For root: stop the iterator (without yielding Position::CloseDir)
                    if cur_depth == 0 {
                        return None;
                    }

                    match self.transition_state {
                        // First step
                        TransitionState::None => {
                            // Just yield Position::CloseDir
                            self.transition_state = TransitionState::BeforePopUp;
                            debug!(self.do_debug_checks());
                            return Position::CloseDir.into_some();
                        }
                        // Second step: surface to parent
                        TransitionState::BeforePopUp => {
                            self.pop_dir();
                            // Clear state
                            self.transition_state = TransitionState::AfterPopUp;
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}
