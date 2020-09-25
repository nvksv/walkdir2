use std::cmp::Ordering;
use std::vec;

use crate::wd::{self, ContentFilter, ContentOrder, Depth, FnCmp, IntoOk, Position};
use crate::fs;
use crate::walk::rawdent::{RawDirEntry, ReadDir};
use crate::cp::ContentProcessor;
use crate::walk::opts::WalkDirOptionsImmut;
use crate::error::{ErrorInner, Error};

/////////////////////////////////////////////////////////////////////////
////

#[derive(Debug)]
pub struct FlatDirEntry<E: fs::FsDirEntry> {
    /// Raw DirEntry
    pub raw: RawDirEntry<E>,
    /// This entry is a dir and will be walked recursive.
    pub is_dir: bool,
    /// This entry is symlink to loop.
    /// - Some(index) => is loop to ancestor[index]
    /// - None => is not loop link
    pub loop_link: Option<Depth>,
}

/////////////////////////////////////////////////////////////////////////
//// DirEntryRecord

#[derive(Debug)]
pub(crate) struct DirEntryRecord<E: fs::FsDirEntry> {
    /// Value from ReadDir
    flat: wd::ResultInner<FlatDirEntry<E>, E>,
    /// This entry must be yielded first according to opts.content_order
    first_pass: bool,
    /// This entry will not be yielded according to opts.content_filter
    hidden: bool,
}

impl<E: fs::FsDirEntry> DirEntryRecord<E> {
    fn new(
        r_rawdent: wd::ResultInner<RawDirEntry<E>, E>,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> Option<Self> {
        let r_flat_dent = match r_rawdent {
            Ok(raw_dent) => match process_rawdent(raw_dent, ctx) {
                Some(flat_dent) => flat_dent,
                None => return None,
            },
            Err(e) => Err(e),
        };

        let this = match r_flat_dent {
            Ok(flat) => {
                let first_pass = match opts_immut.content_order {
                    ContentOrder::None => false,
                    ContentOrder::DirsFirst => flat.is_dir,
                    ContentOrder::FilesFirst => !flat.is_dir,
                };

                let hidden = match opts_immut.content_filter {
                    ContentFilter::None => false,
                    ContentFilter::DirsOnly => !flat.is_dir,
                    ContentFilter::FilesOnly => flat.is_dir,
                    ContentFilter::SkipAll => true,
                };

                Self { flat: Ok(flat), first_pass, hidden }
            }
            Err(err) => Self { flat: Err(err), first_pass: false, hidden: false },
        };

        Some(this)
    }

    fn can_be_yielded(&self) -> bool {
        if !self.hidden {
            return true;
        }

        if let Ok(ref flat) = self.flat {
            return flat.is_dir;
        }

        return false;
    }
}

/////////////////////////////////////////////////////////////////////////
//// DirState

#[derive(Debug)]
pub struct DirContent<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// Source of not consumed DirEntries
    rd: ReadDir<E>,
    /// A list of already consumed DirEntries
    content: Vec<DirEntryRecord<E>>,
    /// Count of consumed entries = position of unconsumed in content
    current_pos: Option<usize>,
    _cp: std::marker::PhantomData<CP>,
}

impl<E, CP> DirContent<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// New DirContent from alone DirEntry
    pub fn new_once(
        raw: RawDirEntry<E>,
    ) -> wd::ResultInner<Self, E> {
        Self {
            rd: ReadDir::<E>::new_once(raw)?,
            content: vec![],
            current_pos: None,
            _cp: std::marker::PhantomData,
        }
        .into_ok()
    }

    /// New DirContent from FsReadDir
    pub fn new(
        parent: &RawDirEntry<E>, 
        ctx: &mut E::Context
    ) -> wd::ResultInner<Self, E> {
        Self {
            rd: parent.read_dir(ctx)?,
            content: vec![],
            current_pos: None,
            _cp: std::marker::PhantomData,
        }
        .into_ok()
    }

    /// Load all remaining DirEntryRecord into tail of self.content.
    /// Doesn't change position.
    pub fn load_all(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) {
        let mut collected = self.rd.collect_all(&mut |r_rawdent, ctx| Self::new_rec(r_rawdent, opts_immut, process_rawdent, ctx), ctx);

        if self.content.is_empty() {
            self.content = collected;
        } else {
            self.content.append(&mut collected);
        }
    }

    /// Makes new DirEntryRecord from processed Result<DirEntry> or rejects it.
    /// Doesn't change position.
    fn new_rec(
        r_rawdent: wd::ResultInner<RawDirEntry<E>, E>,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> Option<DirEntryRecord<E>> {
        let rec = DirEntryRecord::<E>::new(r_rawdent, opts_immut, process_rawdent, ctx)?;

        // if let Ok(ref mut dent) = rec.dent {
        //     dent.set_depth_mut( depth );
        // };

        Some(rec)
    }

    /// Shifts to next record (and loads it when necessary) -- without any passes, content filters and so on.
    /// Updates current position on success.
    pub fn get_next_rec(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> Option<(bool, bool)> {
        loop {
            // Check for already loaded entry
            let next_pos = if let Some(pos) = self.current_pos { pos + 1 } else { 0 };
            if let Some(rec) = self.content.get(next_pos) {
                self.current_pos = Some(next_pos);
                return Some((rec.first_pass, rec.can_be_yielded()));
            }

            if let Some(r_rawdent) = self.rd.next(ctx) {
                let rec = match Self::new_rec(r_rawdent, opts_immut, process_rawdent, ctx) {
                    Some(rec) => rec,
                    None => continue,
                };
                self.content.push(rec);
                self.current_pos = Some(self.content.len() - 1);

                let last = self.content.last();
                assert!(last.is_some());
                let rec = last.unwrap();
                return Some((rec.first_pass, rec.can_be_yielded()));
            }

            break;
        }

        None
    }

    /// Rewind current position: now we stand before beginning.
    pub fn rewind(&mut self) {
        self.current_pos = None;
    }

    /// Gets record at current position
    /// Doesn't change position.
    pub fn get_current_rec(
        &mut self,
        depth: Depth,
    ) -> std::result::Result<FlatDirEntryRef<'_, E, CP>, ErrorInnerRef<'_, E>> {
        let pos = self.current_pos.unwrap();
        let rec = self.content.get_mut(pos).unwrap();

        match rec.flat {
            Ok(ref mut flat) => Ok(FlatDirEntryRef::<E, CP>::new(flat, depth, rec.hidden)),
            Err(ref mut err) => Err(ErrorInnerRef::<E>::new(err, depth)),
        }
    }

    /// Sorts all loaded content.
    /// Changes current position.
    fn sort_content_and_rewind(
        &mut self, 
        cmp: &mut FnCmp<E>, 
        ctx: &mut E::Context,
    ) {
        self.content.sort_by(|a, b| match (&a.flat, &b.flat) {
            (&Ok(ref a), &Ok(ref b)) => RawDirEntry::call_cmp(&a.raw, &b.raw, cmp, ctx),
            (&Err(_), &Err(_)) => Ordering::Equal,
            (&Ok(_), &Err(_)) => Ordering::Greater,
            (&Err(_), &Ok(_)) => Ordering::Less,
        });
        self.current_pos = None;
    }

    /// Sorts all loaded content.
    /// Changes current position.
    pub fn load_all_and_sort(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        cmp: &mut FnCmp<E>,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) {
        self.load_all(opts_immut, process_rawdent, ctx);
        self.sort_content_and_rewind(cmp, ctx);
    }

    // pub fn iter_content<'s, F, T: 's>(&'s self, f: F) -> impl Iterator<Item = &'s T> where F: FnMut(&DirEntryRecord<E>) -> Option<&T> {
    //     self.content.iter().filter_map( f )
    // }

    pub fn iter_content_flats<'s, F, T: 's>(
        &'s mut self, 
        f: F
    ) -> impl Iterator<Item = &'s mut T>
    where
        F: FnMut(&mut FlatDirEntry<E>) -> Option<&mut T>,
    {
        self.content
            .iter_mut()
            .filter_map(|rec: &mut DirEntryRecord<E>| rec.flat.as_mut().ok())
            .filter_map(f)
    }
}

/////////////////////////////////////////////////////////////////////////
//// DirEntryRecordRef

pub struct FlatDirEntryRef<'r, E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    flat: &'r mut FlatDirEntry<E>,
    depth: Depth,
    /// This entry will not be yielded according to opts.content_filter
    hidden: bool,
    _cp: std::marker::PhantomData<CP>,
}

impl<'r, E, CP> FlatDirEntryRef<'r, E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    fn new(flat: &'r mut FlatDirEntry<E>, depth: Depth, hidden: bool) -> Self {
        Self { flat, depth, hidden, _cp: std::marker::PhantomData }
    }

    pub fn make_content_item (
        &mut self,
        content_processor: &mut CP,
        ctx: &mut E::Context,
    ) -> Option<CP::Item> {
        self.flat.raw.make_content_item( content_processor, self.flat.is_dir, self.depth, ctx )
    }

    pub fn as_flat(&self) -> &FlatDirEntry<E> {
        self.flat
    }

    // pub fn depth(&self) -> Depth {
    //     self.depth
    // }

    pub fn is_dir(&self) -> bool {
        self.flat.is_dir
    }

    pub fn hidden(&self) -> bool {
        self.hidden
    }

    pub fn loop_link(&self) -> Option<Depth> {
        self.flat.loop_link
    }

    pub fn path(&self) -> &E::Path {
        self.flat.raw.path()
    }
}

/////////////////////////////////////////////////////////////////////////
//// ErrorInnerRef

pub struct ErrorInnerRef<'r, E: fs::FsDirEntry> {
    err: &'r mut ErrorInner<E>,
    depth: Depth,
}

impl<'r, E: fs::FsDirEntry> ErrorInnerRef<'r, E> {
    fn new(err: &'r mut ErrorInner<E>, depth: Depth) -> Self {
        Self { err, depth }
    }

    pub fn into_error(self) -> Error<E> {
        Error::<E>::from_inner(self.err.take(), self.depth)
    }
}

/////////////////////////////////////////////////////////////////////////
//// DirState

#[derive(Debug, PartialEq, Eq)]
enum DirPass {
    Entire,
    First,
    Second,
}

fn get_initial_pass(opts_immut: &WalkDirOptionsImmut) -> DirPass {
    if opts_immut.content_order == ContentOrder::None {
        DirPass::Entire
    } else {
        DirPass::First
    }
}

#[derive(Debug)]
pub struct DirState<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    /// The depth of this dir
    depth: Depth,
    /// Content of this dir
    content: DirContent<E, CP>,
    /// Current pass
    pass: DirPass,
    /// Current position
    position: Position<(), (), ()>,

    /// Stub
    _cp: std::marker::PhantomData<CP>,
}

impl<E, CP> DirState<E, CP>
where
    E: fs::FsDirEntry,
    CP: ContentProcessor<E>,
{
    fn init(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        sorter: &mut Option<FnCmp<E>>,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) {
        if let Some(cmp) = sorter {
            self.content.load_all_and_sort(opts_immut, cmp, process_rawdent, ctx);
        }
    }

    /// New DirState from alone DirEntry
    pub fn new_once(
        raw: RawDirEntry<E>,
        depth: Depth,
        opts_immut: &WalkDirOptionsImmut,
        sorter: &mut Option<FnCmp<E>>,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> wd::ResultInner<Self, E> {
        let mut this = Self {
            depth,
            content: DirContent::<E, CP>::new_once(raw)?,
            pass: get_initial_pass(opts_immut),
            position: Position::BeforeContent(()),
            _cp: std::marker::PhantomData,
        };
        this.init(opts_immut, sorter, process_rawdent, ctx);
        this.into_ok()
    }

    /// New DirState from FsReadDir
    pub fn new(
        parent: &RawDirEntry<E>,
        depth: Depth,
        opts_immut: &WalkDirOptionsImmut,
        sorter: &mut Option<FnCmp<E>>,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> wd::ResultInner<Self, E> {
        let mut this = Self {
            depth,
            content: DirContent::<E, CP>::new(parent, ctx)?,
            pass: get_initial_pass(opts_immut),
            position: Position::BeforeContent(()),
            _cp: std::marker::PhantomData,
        };
        this.init(opts_immut, sorter, process_rawdent, ctx);
        this.into_ok()
    }

    /// Load all remaining DirEntryRecord into tail of self.content.
    /// Doesn't change position.
    pub fn load_all(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) {
        self.content.load_all(opts_immut, process_rawdent, ctx)
    }

    /// Gets next record (according to content order and filter).
    /// Updates current position.
    fn shift_next(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> bool {
        loop {
            if let Some((first_pass, can_be_yielded)) =
                self.content.get_next_rec(opts_immut, process_rawdent, ctx)
            {
                let valid_pass = match self.pass {
                    DirPass::Entire => true,
                    DirPass::First => first_pass,
                    DirPass::Second => !first_pass,
                };

                if valid_pass && can_be_yielded {
                    return true;
                };

                continue;
            };

            match self.pass {
                DirPass::Entire | DirPass::Second => {
                    self.position = Position::AfterContent;
                    return false;
                }
                DirPass::First => {
                    self.pass = DirPass::Second;
                    self.content.rewind();
                    continue;
                }
            };
        }
    }

    /// Next.
    /// Updates current position.
    pub fn next_position(
        &mut self,
        opts_immut: &WalkDirOptionsImmut,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) {
        if self.position == Position::AfterContent {
            return;
        };

        if self.shift_next(opts_immut, process_rawdent, ctx) {
            // Remember: at this state current rec must exist
            self.position = Position::Entry(());
        } else {
            self.position = Position::AfterContent;
        };
    }

    /// Get current state.
    /// Doesn't change position.
    pub fn get_current_position(
        &mut self,
    ) -> Position<(), FlatDirEntryRef<'_, E, CP>, ErrorInnerRef<'_, E>> {
        match self.position {
            Position::BeforeContent(_) => Position::BeforeContent(()),
            Position::Entry(_) => {
                // At this state current rec must exist
                match self.content.get_current_rec(self.depth) {
                    Ok(flat) => Position::Entry(flat),
                    Err(err) => Position::Error(err),
                }
            }
            Position::AfterContent => Position::AfterContent,
            _ => unreachable!(),
        }
    }

    /// Gets copy of entire dir, loading all remaining content if necessary (not considering content order).
    /// Doesn't change position.
    pub fn clone_all_content(
        &mut self,
        filter: ContentFilter,
        opts_immut: &WalkDirOptionsImmut,
        content_processor: &CP,
        process_rawdent: &mut impl (FnMut(
            RawDirEntry<E>,
            &mut E::Context,
        ) -> Option<wd::ResultInner<FlatDirEntry<E>, E>>),
        ctx: &mut E::Context,
    ) -> CP::Collection {
        self.content.load_all(opts_immut, process_rawdent, ctx);

        let depth = self.depth();

        match filter {
            ContentFilter::None => {
                let iter = self
                    .content
                    .iter_content_flats(|flat| Some(flat))
                    .filter_map(|flat| flat.raw.make_content_item( content_processor, flat.is_dir, depth, ctx ));
                content_processor.collect(iter)
            }
            ContentFilter::DirsOnly => {
                let iter = self
                    .content
                    .iter_content_flats(|flat| if flat.is_dir { Some(flat) } else { None })
                    .filter_map(|flat| flat.raw.make_content_item( content_processor, flat.is_dir, depth, ctx ));
                content_processor.collect(iter)
            }
            ContentFilter::FilesOnly => {
                let iter = self
                    .content
                    .iter_content_flats(|flat| if !flat.is_dir { Some(flat) } else { None })
                    .filter_map(|flat| flat.raw.make_content_item( content_processor, flat.is_dir, depth, ctx ));
                content_processor.collect(iter)
            }
            ContentFilter::SkipAll => CP::empty_collection(),
        }
    }

    pub fn depth(&self) -> Depth {
        self.depth
    }

    pub fn skip_all(&mut self) {
        self.position = Position::AfterContent;
    }
}
