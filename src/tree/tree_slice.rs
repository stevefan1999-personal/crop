use std::ops::Range;
use std::sync::Arc;

use super::{Inode, Leaf, Leaves, Metric, Node, Units};

#[derive(Debug)]
pub struct TreeSlice<'a, const FANOUT: usize, L: Leaf> {
    /// TODO: docs
    root: &'a Arc<Node<FANOUT, L>>,

    /// TODO: docs
    offset: L::Summary,

    /// TODO: docs
    summary: L::Summary,

    /// TODO: docs
    start_slice: &'a L::Slice,

    /// TODO: docs
    start_summary: L::Summary,

    /// TODO: docs
    end_slice: &'a L::Slice,

    /// TODO: docs
    end_summary: L::Summary,
}

impl<'a, const FANOUT: usize, L: Leaf> Clone for TreeSlice<'a, FANOUT, L> {
    #[inline]
    fn clone(&self) -> Self {
        TreeSlice {
            offset: self.offset.clone(),
            summary: self.summary.clone(),
            start_summary: self.start_summary.clone(),
            end_summary: self.end_summary.clone(),
            ..*self
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> Copy for TreeSlice<'a, FANOUT, L> where
    L::Summary: Copy
{
}

impl<'a, const FANOUT: usize, L: Leaf> TreeSlice<'a, FANOUT, L>
where
    for<'d> &'d L::Slice: Default,
{
    /// Note: doesn't do bounds checks.
    #[inline]
    pub(super) fn from_range_in_root<M>(
        root: &'a Arc<Node<FANOUT, L>>,
        range: Range<M>,
    ) -> Self
    where
        M: Metric<L>,
    {
        debug_assert!(M::zero() <= range.start);
        debug_assert!(range.start <= range.end);
        // debug_assert!(range.end <= M::measure(self.summary()));

        let mut tree_slice = Self {
            root,
            offset: L::Summary::default(),
            summary: L::Summary::default(),
            start_slice: Default::default(),
            start_summary: L::Summary::default(),
            end_slice: Default::default(),
            end_summary: L::Summary::default(),
        };

        tree_slice_from_range_in_root_rec(
            root,
            range,
            &mut M::zero(),
            &mut tree_slice,
            &mut false,
            &mut false,
        );

        tree_slice
    }

    /// TODO: docs
    #[inline]
    pub fn leaves(&'a self) -> Leaves<'a, FANOUT, L> {
        Leaves::from(self)
    }

    /// Note: doesn't do bounds checks.
    #[inline]
    pub fn slice<M>(&'a self, range: Range<M>) -> TreeSlice<'a, FANOUT, L>
    where
        M: Metric<L>,
    {
        debug_assert!(M::zero() <= range.start);
        debug_assert!(range.start <= range.end);
        // debug_assert!(range.end <= M::measure(self.summary()));

        let mut tree_slice = Self {
            root: self.root,
            offset: L::Summary::default(),
            summary: L::Summary::default(),
            start_slice: Default::default(),
            start_summary: L::Summary::default(),
            end_slice: Default::default(),
            end_summary: L::Summary::default(),
        };

        tree_slice_from_range_in_root_rec(
            self.root,
            range,
            &mut M::measure(&self.offset),
            &mut tree_slice,
            &mut false,
            &mut false,
        );

        tree_slice
    }

    #[inline]
    pub fn summary(&self) -> &L::Summary {
        &self.summary
    }

    /// TODO: docs
    #[inline]
    pub fn units<M>(&'a self) -> Units<'a, FANOUT, L, M>
    where
        M: Metric<L>,
    {
        Units::from(self)
    }
}

#[inline]
fn tree_slice_from_range_in_root_rec<'a, const N: usize, L, M>(
    node: &'a Arc<Node<N, L>>,
    range: Range<M>,
    measured: &mut M,
    slice: &mut TreeSlice<'a, N, L>,
    found_start: &mut bool,
    done: &mut bool,
) where
    L: Leaf,
    M: Metric<L>,
{
    match &**node {
        Node::Internal(inode) => {
            for child in inode.children() {
                // If the slice has been completed there's nothing left to do,
                // simply unwind the call stack.
                if *done {
                    return;
                }

                let measure = M::measure(child.summary());

                if !*found_start {
                    if *measured + measure > range.start {
                        // If the child contains the start of the range but not
                        // the end then `node` is the deepest node that fully
                        // contains the tree slice.
                        if !(*measured + measure >= range.end) {
                            slice.root = node;
                        }
                        // This child contains the starting slice somewhere in
                        // its subtree. Run this function again with this child
                        // as the node.
                        tree_slice_from_range_in_root_rec(
                            child,
                            Range { start: range.start, end: range.end },
                            measured,
                            slice,
                            found_start,
                            done,
                        )
                    } else {
                        // This child comes before the starting leaf.
                        slice.offset += child.summary();
                        *measured += measure;
                    }
                } else if *measured + measure >= range.end {
                    // This child contains the ending leaf somewhere in its
                    // subtree. Run this function again with this child as the
                    // node.
                    tree_slice_from_range_in_root_rec(
                        child,
                        Range { start: range.start, end: range.end },
                        measured,
                        slice,
                        found_start,
                        done,
                    )
                } else {
                    // This is a node fully contained between the starting and
                    // the ending slices.
                    slice.summary += child.summary();
                    *measured += measure;
                }
            }
        },

        Node::Leaf(leaf) => {
            let measure = M::measure(leaf.summary());

            if !*found_start {
                if *measured + measure > range.start {
                    // This leaf contains the starting slice.
                    if measure >= range.end - *measured {
                        // The end of the range is also contained in this leaf
                        // so the final slice only spans this single leaf.
                        let (start_slice, start_summary) = M::slice(
                            leaf.slice(),
                            range.start - *measured..range.end - *measured,
                            leaf.summary(),
                        );
                        slice.root = node;
                        slice.summary = start_summary.clone();
                        slice.start_slice = start_slice;
                        slice.start_summary = start_summary;
                        *done = true;
                    } else {
                        let (_, start_slice, start_summary) = M::split_right(
                            leaf.slice(),
                            range.start - *measured,
                            leaf.summary(),
                        );
                        slice.summary = start_summary.clone();
                        slice.start_slice = start_slice;
                        slice.start_summary = start_summary;
                        *found_start = true;
                    }
                } else {
                    // This leaf comes before the starting leaf.
                    slice.offset += leaf.summary();
                    *measured += measure;
                }
            } else if *measured + measure >= range.end {
                // This leaf contains the ending slice.
                let (end_slice, end_summary, _) = M::split_left(
                    leaf.slice(),
                    range.end - *measured,
                    leaf.summary(),
                );
                slice.summary += &end_summary;
                slice.end_slice = end_slice;
                slice.end_summary = end_summary;
                *done = true;
            } else {
                // This is a leaf between the starting and the ending slices.
                slice.summary += leaf.summary();
                *measured += measure;
            }
        },
    }
}
