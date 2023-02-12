use std::ops::Range;
use std::sync::Arc;

use super::*;

/// A self-balancing tree with metadata stored in each node.
#[derive(Default)]
pub struct Tree<const FANOUT: usize, L: Leaf> {
    pub(super) root: Arc<Node<FANOUT, L>>,
}

impl<const N: usize, L: Leaf> Clone for Tree<N, L> {
    #[inline]
    fn clone(&self) -> Self {
        Tree { root: Arc::clone(&self.root) }
    }
}

impl<const N: usize, L: Leaf> std::fmt::Debug for Tree<N, L> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if !f.alternate() {
            f.debug_struct("Tree").field("root", &self.root).finish()
        } else {
            write!(f, "{:#?}", self.root)
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> From<TreeSlice<'a, FANOUT, L>>
    for Tree<FANOUT, L>
{
    #[inline]
    fn from(slice: TreeSlice<'a, FANOUT, L>) -> Tree<FANOUT, L> {
        let root = if slice.base_measure() == slice.root().base_measure() {
            // If the TreeSlice and its root have the same base measure it
            // means the TreeSlice spanned the whole Tree from which it was
            // created and we can simply clone the root.
            Arc::clone(slice.root())
        } else if slice.leaf_count() == 1 {
            debug_assert!(slice.root().is_leaf());

            Arc::new(Node::Leaf(Lnode::new(
                slice.first_slice.to_owned(),
                slice.summary,
            )))
        } else if slice.leaf_count() == 2 {
            let (first, second) = L::balance_slices(
                (slice.first_slice, &slice.first_summary),
                (slice.last_slice, &slice.last_summary),
            );

            let first = Arc::new(Node::Leaf(Lnode::from(first)));

            if let Some(second) = second {
                let second = Arc::new(Node::Leaf(Lnode::from(second)));
                let root = Inode::from_children([first, second]);
                Arc::new(Node::Internal(root))
            } else {
                first
            }
        } else {
            return from_treeslice::into_tree_root(slice);
        };

        #[cfg(debug_assertions)]
        (Tree { root: Arc::clone(&root) }).assert_invariants();

        Tree { root }
    }
}

impl<const FANOUT: usize, L: Leaf> Tree<FANOUT, L> {
    /*
      Public methods
    */

    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        match &*self.root {
            Node::Internal(root) => {
                // The root is the only inode that can have as few as 2
                // children.
                assert!(
                    root.children().len() >= 2
                        && root.children().len() <= FANOUT
                );

                for child in root.children() {
                    child.assert_invariants()
                }
            },

            Node::Leaf(leaf) => {
                assert_eq!(&leaf.value.summarize(), self.summary());
            },
        }
    }

    /// Returns the base measure of this `Tree` obtained by summing up the
    /// base measures of all its leaves.
    #[inline]
    pub fn base_measure(&self) -> L::BaseMetric {
        self.measure::<L::BaseMetric>()
    }

    /// Returns the `M2`-measure of all the leaves before `up_to` plus the
    /// `M2`-measure of the left sub-slice of the leaf at `up_to`.
    ///
    /// NOTE: this function doesn't do any bounds checks.
    #[inline]
    pub fn convert_measure<M1, M2>(&self, up_to: M1) -> M2
    where
        M1: SlicingMetric<L>,
        M2: Metric<L>,
    {
        debug_assert!(up_to <= self.measure::<M1>() + M1::one(),);
        self.root.convert_measure(up_to)
    }

    /// Creates a new `Tree` from a collection of leaves.
    ///
    /// NOTE: if the iterator yields 0 items the `Tree` will contain a single
    /// leaf with `L`'s default value.
    #[inline]
    pub fn from_leaves<I>(leaves: I) -> Self
    where
        I: IntoIterator<Item = L>,
        L: Default,
    {
        Self {
            root: Self::root_from_leaves(leaves.into_iter())
                .unwrap_or_default(),
        }
    }

    /// Returns the leaf containing the `measure`-th unit of the `M`-metric,
    /// plus the `M`-measure of all the leaves before it.
    ///
    /// NOTE: this function doesn't do any bounds checks.
    #[inline]
    pub fn leaf_at_measure<M>(&self, measure: M) -> (&L::Slice, M)
    where
        M: Metric<L>,
    {
        debug_assert!(measure <= self.measure::<M>() + M::one());

        self.root.leaf_at_measure(measure)
    }

    #[inline]
    pub fn leaf_count(&self) -> usize {
        self.root.leaf_count()
    }

    /// Returns an iterator over the leaves of this `Tree`.
    #[inline]
    pub fn leaves(&self) -> Leaves<'_, FANOUT, L> {
        Leaves::from(self)
    }

    /// Returns the `M`-measure of this `Tree` obtaining by summing up the
    /// `M`-measures of all its leaves.
    #[inline]
    pub fn measure<M: Metric<L>>(&self) -> M {
        M::measure(self.summary())
    }

    /// TODO: docs
    #[inline]
    pub fn replace<'a, M, I>(&mut self, range: Range<M>, slices: I)
    where
        L: ReplaceableLeaf<M>,
        M: Metric<L>,
        I: IntoIterator<Item = &'a L::Slice>,
        L::Slice: 'a,
        for<'d> &'d L::Slice: Default,
    {
        let root = Arc::make_mut(&mut self.root);

        let split = tree_replace::some_name(root, range, slices.into_iter());

        if let Some(split) = split {
            todo!();
        }

        // TODO:
        // - consume the iterator, build the Tree
        //
        // - nodes in push back should have the same depth as the root (or maybe
        // one less?)
        //
        // - if the depth of the Tree is the same as the root create a new
        // Inode with the root, the new tree and the nodes in push_back.
        //
        // - if it's < do an append_at depth on the root, then same as before
        // with the nodes in push_back
        //
        // - if it's > 0 do a prepend at depth on that node with the root, then
        // an append at depth with the nodes in push_back.

        // if !slices.is_empty() {
        //     todo!();
        // }

        // if let Some(split) = split {
        //     let old_root = std::mem::replace(
        //         root,
        //         Node::Internal(Inode::from_children([split])),
        //     );

        //     unsafe {
        //         root.as_mut_internal_unchecked().insert(0, Arc::new(old_root))
        //     }
        // }

        #[cfg(debug_assertions)]
        self.assert_invariants();
    }

    /// TODO: docs
    #[inline]
    fn root_from_leaves<I>(mut leaves: I) -> Option<Arc<Node<FANOUT, L>>>
    where
        I: Iterator<Item = L>,
    {
        let first = leaves.next()?;
        let first = Arc::new(Node::Leaf(Lnode::from(first)));

        let Some(second) = leaves.next() else { return Some(first) };
        let second = Arc::new(Node::Leaf(Lnode::from(second)));

        let mut nodes = {
            let (lo, hi) = leaves.size_hint();
            let mut n = Vec::with_capacity(2 + hi.unwrap_or(lo));
            n.push(first);
            n.push(second);
            n.extend(leaves.map(Lnode::from).map(Node::Leaf).map(Arc::new));
            n
        };

        while nodes.len() > FANOUT {
            let capacity =
                nodes.len() / FANOUT + ((nodes.len() % FANOUT != 0) as usize);

            let mut new_nodes = Vec::with_capacity(capacity);

            let mut iter = nodes.into_iter();

            while iter.len() > 0 {
                let children = iter.by_ref().take(FANOUT);
                let inode = Inode::from_children(children);
                new_nodes.push(Arc::new(Node::Internal(inode)));
            }

            nodes = new_nodes;
        }

        let mut root = Inode::from_children(nodes);

        root.balance_right_side();

        let mut tree = Self { root: Arc::new(Node::Internal(root)) };

        tree.pull_up_root();

        Some(tree.root)
    }

    /// Returns a slice of the `Tree` in the range of the given metric.
    #[inline]
    pub fn slice<M>(&self, range: Range<M>) -> TreeSlice<'_, FANOUT, L>
    where
        M: SlicingMetric<L>,
        L::BaseMetric: SlicingMetric<L>,
        for<'d> &'d L::Slice: Default,
    {
        debug_assert!(M::zero() <= range.start);
        debug_assert!(range.start <= range.end);
        debug_assert!(range.end <= self.measure::<M>() + M::one());

        TreeSlice::from_range_in_root(&self.root, range)
    }

    #[inline]
    pub fn summary(&self) -> &L::Summary {
        self.root.summary()
    }

    /// Returns an iterator over the `M`-units of this `Tree`.
    #[inline]
    pub fn units<M>(&self) -> Units<'_, FANOUT, L, M>
    where
        M: Metric<L>,
        for<'d> &'d L::Slice: Default,
    {
        Units::from(self)
    }

    /*
      Private methods
    */

    /// Continuously replaces the root with its first child as long as the root
    /// is an internal node with a single child.
    ///
    /// # Panics
    ///
    /// Panics if the `Arc` enclosing the root has a strong counter > 1.
    #[inline]
    pub(super) fn pull_up_root(&mut self) {
        let root = &mut self.root;

        while let Node::Internal(i) = Arc::get_mut(root).unwrap() {
            if i.children().len() == 1 {
                let child = unsafe {
                    i.children
                        .drain(..)
                        .next()
                        // SAFETY: there is exactly 1 child.
                        .unwrap_unchecked()
                };
                *root = child;
            } else {
                break;
            }
        }
    }

    #[inline]
    pub(super) fn root(&self) -> &Arc<Node<FANOUT, L>> {
        &self.root
    }
}

mod tree_replace {
    //! This module handles the logic used in [`Tree::replace()`].

    use super::*;

    /// This is recursive.
    ///
    /// TODO: docs
    #[inline]
    pub(super) fn some_name<'a, const N: usize, L, M, I>(
        node: &mut Node<N, L>,
        mut range: Range<M>,
        mut slices: I,
    ) -> Option<Vec<Arc<Node<N, L>>>>
    where
        L: ReplaceableLeaf<M>,
        M: Metric<L>,
        I: Iterator<Item = &'a L::Slice>,
        L::Slice: 'a,
        for<'d> &'d L::Slice: Default,
    {
        // 1: get to the deepest node that fully contains the replaced range.

        match node {
            Node::Internal(inode) => {
                let (mut child_idx, split) = {
                    let mut measured = M::zero();
                    let mut children = inode.children().iter().enumerate();

                    loop {
                        let (idx, child) = children.next().unwrap();

                        let child_measure = child.measure::<M>();

                        let contains_start =
                            measured + child_measure >= range.start;

                        if contains_start {
                            let contains_last_slice =
                                measured + child_measure >= range.end;

                            if contains_last_slice {
                                range.start -= measured;
                                range.end -= measured;
                                let split =
                                    inode.with_child_mut(idx, |child| {
                                        some_name(
                                            Arc::make_mut(child),
                                            range,
                                            slices,
                                        )
                                    });
                                break (idx, split);
                            } else {
                                return do_stuff_with_deepest(
                                    inode, range, slices,
                                );
                            };
                        } else {
                            measured += child_measure;
                        }
                    }
                };

                if let Some(split) = split {
                    for node in split {
                        inode.insert(child_idx + 1, node);
                        child_idx += 1;
                    }
                }

                if inode.is_overfull() {
                    let min_children = Inode::<N, L>::min_children();
                    let split_offset = inode.children().len() - min_children;
                    let split = inode.split_off(split_offset);
                    Some(vec![Arc::new(Node::Internal(split))])
                } else {
                    None
                }
            },

            Node::Leaf(leaf) => {
                let replace_with = slices.next().unwrap_or_default();

                let extra = leaf.replace(range, replace_with);

                let leaves = slices
                    .map(std::borrow::ToOwned::to_owned)
                    .map(Lnode::from)
                    .map(Node::Leaf)
                    .map(Arc::new);

                if let Some(extra) = extra {
                    let (lo, hi) = leaves.size_hint();
                    let mut nodes = Vec::with_capacity(1 + hi.unwrap_or(lo));
                    nodes.push(Arc::new(Node::Leaf(extra)));
                    nodes.extend(leaves);
                    Some(nodes)
                } else {
                    let nodes = leaves.collect::<Vec<_>>();
                    (!nodes.is_empty()).then_some(nodes)
                }
            },
        }
    }

    /// This is not recursive. The only the code that handles the first final
    /// branches are.
    ///
    /// Returns a vector of nodes that should be pushed back onto the Tree
    /// after this node.
    ///
    /// TODO: docs
    #[inline]
    fn do_stuff_with_deepest<'a, const N: usize, L, M, I>(
        inode: &mut Inode<N, L>,
        range: Range<M>,
        mut slices: I,
    ) -> Option<Vec<Arc<Node<N, L>>>>
    where
        L: ReplaceableLeaf<M>,
        M: Metric<L>,
        I: Iterator<Item = &'a L::Slice>,
        L::Slice: 'a,
        for<'d> &'d L::Slice: Default,
    {
        // The index of the child containing the start of the replacement
        // range.
        let mut start_idx = 0;

        let mut split_first = None;

        let mut offset = M::zero();

        let mut child_indexes = 0..inode.children().len();

        for (idx, child) in
            child_indexes.by_ref().map(|idx| (idx, &inode.children[idx]))
        {
            let child_measure = child.measure::<M>();

            offset += child_measure;

            if offset >= range.start {
                start_idx = idx;

                split_first = inode.with_child_mut(start_idx, |child| {
                    replace_first_rec(
                        Arc::make_mut(child),
                        range.start + child_measure - offset,
                        &mut slices,
                    )
                });

                break;
            }
        }

        // The index of the child containing the end of the replacement range.
        let mut end_idx = 0;

        let mut split_last = None;

        let mut slices = slices.collect::<Vec<_>>();

        for (idx, child) in
            child_indexes.map(|idx| (idx, &inode.children[idx]))
        {
            let child_measure = child.measure::<M>();

            offset += child_measure;

            if offset >= range.end {
                end_idx = idx;

                split_last = inode.with_child_mut(end_idx, |child| {
                    replace_last_rec(
                        Arc::make_mut(child),
                        range.end + child_measure - offset,
                        &mut slices,
                    )
                });

                break;
            }
        }

        if let Some(split_first) = split_first {
            if split_first.depth() + 1 == inode.depth() {
                if start_idx + 1 < end_idx {
                    start_idx += 1;
                    inode.swap_child(start_idx, split_first);
                } else {
                    end_idx += 1;
                    inode.insert(start_idx, split_first);
                }
            } else {
                todo!();
            }
        }

        if let Some(split_last) = split_last {
            if split_last.depth() + 1 == inode.depth() {
                inode.insert(end_idx + 1, split_last);
            } else {
                todo!();
            }
        }

        let leaf_count = (inode.depth() - 1) ^ N;

        let mut leaves = slices.drain(..).map(std::borrow::ToOwned::to_owned);

        for idx in start_idx + 1..end_idx {
            if let Some(node) =
                Tree::root_from_leaves(leaves.by_ref().take(leaf_count))
            {
                if node.depth() + 1 == inode.depth() {
                    inode.swap_child(idx, node);
                } else {
                    todo!();
                }
            } else {
                inode.remove(idx)
            }
        }

        while let Some(node) =
            Tree::root_from_leaves(leaves.by_ref().take(leaf_count))
        {
            if node.depth() + 1 == inode.depth() {
                inode.insert(end_idx, node);
                end_idx += 1;
            } else {
                todo!();
            }
        }

        if inode.is_overfull() {
            let min_children = Inode::<N, L>::min_children();
            let split_offset = inode.children().len() - min_children;
            let split = inode.split_off(split_offset);
            Some(vec![Arc::new(Node::Internal(split))])
        } else {
            None
        }
    }

    /// TODO: docs
    #[inline]
    fn replace_first_rec<'a, const N: usize, L, M, I>(
        node: &mut Node<N, L>,
        replace_from: M,
        slices: &mut I,
    ) -> Option<Arc<Node<N, L>>>
    where
        L: ReplaceableLeaf<M>,
        M: Metric<L>,
        I: Iterator<Item = &'a L::Slice>,
        L::Slice: 'a,
        for<'d> &'d L::Slice: Default,
    {
        match node {
            Node::Internal(inode) => {
                let mut offset = M::zero();

                for (idx, child) in inode.children().iter().enumerate() {
                    let child_measure = child.measure::<M>();

                    if offset + child_measure >= replace_from {
                        let extra = inode.with_child_mut(idx, |child| {
                            replace_first_rec(
                                Arc::make_mut(child),
                                replace_from - offset,
                                slices,
                            )
                        });

                        // If the child was the last in the inode there's
                        // nothing left to do.
                        if idx + 1 == inode.children().len() {
                            return extra;
                        }

                        // TODO: prepend_at_depth `extra` on the next node.

                        for idx in idx + 1..inode.children().len() {
                            let leaf_count = (inode.depth() - 1) ^ N;

                            let replacement = Tree::root_from_leaves(
                                slices
                                    .take(leaf_count)
                                    .map(std::borrow::ToOwned::to_owned),
                            );

                            if let Some(replacement) = replacement {
                                if replacement.depth() + 1 == inode.depth() {
                                    inode.swap_child(idx, replacement);
                                } else {
                                    // prepend at depth on the next or return
                                    // it.
                                    todo!();
                                }
                            } else {
                                // inode.drain(idx..inode.children().len());
                                return None;
                            }
                        }

                        todo!();
                    } else {
                        offset += child_measure;
                    }
                }

                unreachable!();
            },

            Node::Leaf(leaf) => {
                let replace_with = slices.next().unwrap_or_default();

                // NOTE: the end of the range is wrong for metrics other than
                // the BaseMetric because there might be some `M`-remainder
                // that is not part of the range. This is not a problem for now
                // because `Rope::replace()` uses the ByteMetric.
                //
                // TODO: make `Leaf::replace()` generic over a `RangeBounds<M>`
                // instead of `Range<M>`?
                return leaf
                    .replace(replace_from..leaf.measure::<M>(), replace_with)
                    .map(|e| Arc::new(Node::Leaf(e)));
            },
        }
    }

    /// TODO: docs
    #[inline]
    fn replace_last_rec<'a, const N: usize, L, M>(
        node: &mut Node<N, L>,
        replace_up_to: M,
        slices: &mut Vec<&'a L::Slice>,
    ) -> Option<Arc<Node<N, L>>>
    where
        L: ReplaceableLeaf<M>,
        M: Metric<L>,
        L::Slice: 'a,
        for<'d> &'d L::Slice: Default,
    {
        match node {
            Node::Internal(inode) => {
                let mut offset = M::zero();

                for (idx, child) in inode.children().iter().enumerate() {
                    let child_measure = child.measure::<M>();

                    if offset + child_measure >= replace_up_to {
                        inode.with_child_mut(idx, |child| {
                            replace_last_rec(
                                Arc::make_mut(child),
                                replace_up_to - offset,
                                slices,
                            )
                        });

                        todo!();
                    } else {
                        offset += child_measure;
                        // TODO: we gotta do some shit here.
                    }
                }

                unreachable!();
            },

            Node::Leaf(leaf) => {
                let replace_with = slices.pop().unwrap_or_default();

                return leaf
                    .replace(M::zero()..replace_up_to, replace_with)
                    .map(|e| Arc::new(Node::Leaf(e)));
            },
        }
    }
}

mod from_treeslice {
    //! This module handles the logic used to convert `TreeSlice`s into
    //! `Tree`s.

    use super::*;

    /// Converts a `TreeSlice` into the root of an equivalent `Tree`.
    ///
    /// NOTE: can only be called if the slice has a leaf count of at least 3.
    /// Leaf counts of 1 or 2 should be handled before calling this function.
    #[inline]
    pub(super) fn into_tree_root<const N: usize, L: Leaf>(
        slice: TreeSlice<'_, N, L>,
    ) -> Tree<N, L> {
        debug_assert!(slice.leaf_count() >= 3);

        let (root, invalid_in_first, invalid_in_last) = cut_tree_slice(slice);

        let mut tree = Tree { root: Arc::new(Node::Internal(root)) };

        if invalid_in_first > 0 {
            {
                // Safety : `root` was just enclosed in a `Node::Internal`
                // variant.
                let root = unsafe {
                    Arc::get_mut(&mut tree.root)
                        .unwrap()
                        .as_mut_internal_unchecked()
                };

                root.balance_left_side();
            }

            tree.pull_up_root();
        }

        if invalid_in_last > 0 {
            {
                // Safety (as_mut_internal_unchecked): for the root to become a
                // leaf node after the previous call to `pull_up_singular` the
                // TreeSlice would've had to span 2 leaves, and that case case
                // should have already been handled before calling this
                // function.
                let root = unsafe {
                    Arc::get_mut(&mut tree.root)
                        .unwrap()
                        .as_mut_internal_unchecked()
                };

                root.balance_right_side();
            }

            tree.pull_up_root();
        }

        #[cfg(debug_assertions)]
        tree.assert_invariants();

        tree
    }

    /// Returns a `(Root, InvalidFirst, InvalidLast)` tuple where:
    ///
    /// - `Root`: the internal node obtained by removing all the nodes before
    /// `slice.before` and after `slice.before + slice.base_measure`,
    ///
    /// - `Invalid{First,Last}`: the number of invalid nodes contained in the
    /// subtree of the first and last child, respectively.
    ///
    /// NOTE: this function can only be called if the slice has a leaf count of
    /// at least 3.
    ///
    /// NOTE: `Root` is guaranteed to have the same depth as the root of the
    /// slice.
    ///
    /// NOTE: `Root` is guaranteed to have at least 2 children.
    ///
    /// NOTE: both `InvalidFirst` and `InvalidLast` are guaranteed to be less
    /// than or equal to the depth of `Root`.
    ///
    /// NOTE: the `Arc` enclosing the first and last children all the way to
    /// the bottom of the inode are guaranteed to have a strong count of 1, so
    /// it's ok to call `Arc::get_mut` on them. The nodes in the middle will
    /// usually be `Arc::clone`d from the slice.
    #[inline]
    fn cut_tree_slice<const N: usize, L: Leaf>(
        slice: TreeSlice<'_, N, L>,
    ) -> (Inode<N, L>, usize, usize) {
        debug_assert!(slice.leaf_count() >= 3);

        let mut root = Inode::empty();
        let mut invalid_first = 0;
        let mut invalid_last = 0;

        let mut offset = L::BaseMetric::zero();

        let mut children = {
            // Safety: the slice's leaf count is > 1 so its root has to be an
            // internal node.
            let root = unsafe { slice.root().as_internal_unchecked() };
            root.children().iter()
        };

        let start = L::BaseMetric::measure(&slice.offset);

        for child in children.by_ref() {
            let this = child.base_measure();

            if offset + this > start {
                if start == L::BaseMetric::zero() {
                    root.push(Arc::clone(child));
                } else {
                    let first = cut_first_rec(
                        child,
                        start - offset,
                        slice.first_slice,
                        slice.first_summary.clone(),
                        &mut invalid_first,
                    );

                    root.push(first);
                }

                offset += this;
                break;
            } else {
                offset += this;
            }
        }

        let end = start + slice.base_measure();

        for child in children {
            let this = child.base_measure();

            if offset + this >= end {
                if end == slice.root().base_measure() {
                    root.push(Arc::clone(child));
                } else {
                    let last = cut_last_rec(
                        child,
                        end - offset,
                        slice.last_slice,
                        slice.last_summary.clone(),
                        &mut invalid_last,
                    );

                    root.push(last);
                }

                break;
            } else {
                root.push(Arc::clone(child));
                offset += this;
            }
        }

        (root, invalid_first, invalid_last)
    }

    #[inline]
    fn cut_first_rec<const N: usize, L: Leaf>(
        node: &Arc<Node<N, L>>,
        take_from: L::BaseMetric,
        start_slice: &L::Slice,
        start_summary: L::Summary,
        invalid_nodes: &mut usize,
    ) -> Arc<Node<N, L>> {
        match &**node {
            Node::Internal(i) => {
                let mut inode = Inode::empty();

                let mut offset = L::BaseMetric::zero();

                let mut children = i.children().iter();

                for child in children.by_ref() {
                    let this = child.base_measure();

                    if offset + this > take_from {
                        let first = cut_first_rec(
                            child,
                            take_from - offset,
                            start_slice,
                            start_summary,
                            invalid_nodes,
                        );

                        let first_is_valid = first.is_valid();

                        inode.push(first);

                        for child in children {
                            inode.push(Arc::clone(child));
                        }

                        if !first_is_valid && inode.children().len() > 1 {
                            inode.balance_first_child_with_second();
                            *invalid_nodes -= 1;
                        }

                        if !inode.has_enough_children() {
                            *invalid_nodes += 1;
                        }

                        return Arc::new(Node::Internal(inode));
                    } else {
                        offset += this;
                    }
                }

                unreachable!();
            },

            Node::Leaf(_) => {
                let lnode = Lnode::new(start_slice.to_owned(), start_summary);

                if !lnode.is_big_enough() {
                    *invalid_nodes += 1;
                }

                Arc::new(Node::Leaf(lnode))
            },
        }
    }

    #[inline]
    fn cut_last_rec<const N: usize, L: Leaf>(
        node: &Arc<Node<N, L>>,
        take_up_to: L::BaseMetric,
        end_slice: &L::Slice,
        end_summary: L::Summary,
        invalid_nodes: &mut usize,
    ) -> Arc<Node<N, L>> {
        match &**node {
            Node::Internal(i) => {
                let mut inode = Inode::empty();

                let mut offset = L::BaseMetric::zero();

                for child in i.children() {
                    let this = child.base_measure();

                    if offset + this >= take_up_to {
                        let last = cut_last_rec(
                            child,
                            take_up_to - offset,
                            end_slice,
                            end_summary,
                            invalid_nodes,
                        );

                        let last_is_valid = last.is_valid();

                        inode.push(last);

                        if !last_is_valid && inode.children().len() > 1 {
                            inode.balance_last_child_with_penultimate();
                            *invalid_nodes -= 1;
                        }

                        if !inode.has_enough_children() {
                            *invalid_nodes += 1;
                        }

                        return Arc::new(Node::Internal(inode));
                    } else {
                        inode.push(Arc::clone(child));
                        offset += this;
                    }
                }

                unreachable!();
            },

            Node::Leaf(_) => {
                let lnode = Lnode::new(end_slice.to_owned(), end_summary);

                if !lnode.is_big_enough() {
                    *invalid_nodes = 1;
                }

                Arc::new(Node::Leaf(lnode))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{Add, AddAssign, Sub, SubAssign};

    use super::*;
    use crate::tree::Summarize;

    #[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
    pub struct Count {
        count: usize,
        leaves: usize,
    }

    impl Add<&Self> for Count {
        type Output = Self;

        #[inline]
        fn add(self, rhs: &Self) -> Self {
            Count {
                count: self.count + rhs.count,
                leaves: self.leaves + rhs.leaves,
            }
        }
    }

    impl Sub<&Self> for Count {
        type Output = Self;

        #[inline]
        fn sub(self, rhs: &Self) -> Self {
            Count {
                count: self.count - rhs.count,
                leaves: self.leaves - rhs.leaves,
            }
        }
    }

    impl<'a> AddAssign<&'a Self> for Count {
        fn add_assign(&mut self, rhs: &'a Self) {
            self.count += rhs.count;
            self.leaves += rhs.leaves;
        }
    }

    impl<'a> SubAssign<&'a Self> for Count {
        fn sub_assign(&mut self, rhs: &'a Self) {
            self.count -= rhs.count;
            self.leaves -= rhs.leaves;
        }
    }

    impl Summarize for usize {
        type Summary = Count;

        fn summarize(&self) -> Self::Summary {
            Count { count: *self, leaves: 1 }
        }
    }

    type LeavesMetric = usize;

    impl Metric<usize> for LeavesMetric {
        fn zero() -> Self {
            0
        }

        fn one() -> Self {
            1
        }

        fn measure(count: &Count) -> Self {
            count.leaves
        }
    }

    impl Leaf for usize {
        type BaseMetric = LeavesMetric;
        type Slice = Self;
    }

    #[test]
    fn easy() {
        let tree = Tree::<4, usize>::from_leaves(0..20);
        assert_eq!(190, tree.summary().count);
    }

    // #[test]
    // fn slice() {
    //     let tree = Tree::<4, usize>::from_leaves(0..20);
    //     assert_eq!(10, tree.slice(1..5).summary().count);
    // }
}
