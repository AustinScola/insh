//! How the panes of a window are arranged, and where that puts each of them.

use insh_api::{PaneLayout, Split};
use rend::{Location, Size};

use uuid::Uuid;

/// How the panes of a window are arranged.
///
/// Splitting a pane which is already one of several arranged the same way puts the new one in
/// among them rather than nesting another split inside, which is what keeps splitting the same
/// pane again and again from making an ever deeper tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    /// One pane, taking up everything it is given.
    Pane(Uuid),
    /// Several parts, one beside the next or one above the next.
    Split {
        /// Which way they are arranged.
        split: Split,
        /// The parts, in the order they are shown in.
        parts: Vec<Tree>,
        /// How many rows or columns each part is to get.
        ///
        /// These are shared out in proportion rather than used as they are, so that a window which
        /// changes size keeps the parts of it looking the way they were left. Dragging the line
        /// between two parts is what changes them.
        sizes: Vec<usize>,
    },
}

impl Tree {
    /// How many rows or columns the line drawn between two parts takes up.
    pub const DIVIDER: usize = 1;

    /// The fewest rows or columns a pane is given.
    const LEAST: usize = 1;

    /// What a part of a split is to get before the line between them has been dragged.
    ///
    /// The number itself means nothing, since the parts are shared out in proportion to each
    /// other; two parts of the same share are half the window each.
    const SHARE: usize = 1000;

    /// Return where every pane is, given the corner the window starts at and how big it is.
    pub fn layout(&self, at: Location, size: Size) -> Vec<PaneLayout> {
        let mut layouts: Vec<PaneLayout> = Vec::new();
        self.lay_out(at, size, &mut layouts);
        return layouts;
    }

    /// Put where every part is onto the given layouts.
    fn lay_out(&self, at: Location, size: Size, layouts: &mut Vec<PaneLayout>) {
        let (split, parts, sizes): (Split, &Vec<Tree>, &Vec<usize>) = match self {
            Self::Pane(pane) => {
                layouts.push(PaneLayout::builder().pane(*pane).at(at).size(size).build());
                return;
            }
            Self::Split {
                split,
                parts,
                sizes,
            } => (*split, parts, sizes),
        };

        let along: usize = match split {
            Split::Beside => size.columns,
            Split::Below => size.rows,
        };

        for (part, (start, len)) in parts.iter().zip(Self::shares(along, sizes)) {
            let (at, size): (Location, Size) = match split {
                Split::Beside => (
                    Location::builder()
                        .row(at.row)
                        .column(at.column + start)
                        .build(),
                    Size::new(size.rows, len),
                ),
                Split::Below => (
                    Location::builder()
                        .row(at.row + start)
                        .column(at.column)
                        .build(),
                    Size::new(len, size.columns),
                ),
            };
            part.lay_out(at, size, layouts);
        }
    }

    /// Return where each part starts and how much of the rows or columns it gets.
    ///
    /// The line drawn between two parts takes up a row or a column of its own, and what is left is
    /// shared out in proportion to the sizes the parts are to get. Whatever does not divide evenly
    /// goes to the parts at the start, so that none of the screen is left unused.
    fn shares(along: usize, sizes: &[usize]) -> Vec<(usize, usize)> {
        let parts: usize = sizes.len();
        let dividers: usize = Self::DIVIDER * parts.saturating_sub(1);
        let shared: usize = along.saturating_sub(dividers);

        let total: usize = sizes.iter().sum::<usize>().max(1);

        let mut lengths: Vec<usize> = sizes
            .iter()
            .map(|size| ((size * shared) / total).max(Self::LEAST))
            .collect();

        // NOTE: Rounding down leaves some over, which goes to the parts at the start a row or a
        // column at a time. Rounding up instead would ask for more of the screen than there is.
        let mut over: usize = shared.saturating_sub(lengths.iter().sum());
        for length in lengths.iter_mut() {
            if over == 0 {
                break;
            }
            *length += 1;
            over -= 1;
        }

        let mut shares: Vec<(usize, usize)> = Vec::with_capacity(parts);
        let mut start: usize = 0;
        for length in lengths {
            shares.push((start, length));
            start += length + Self::DIVIDER;
        }

        return shares;
    }

    /// Set the size of every part to the rows or columns it is being given.
    ///
    /// The sizes are shared out in proportion, so they mean nothing on their own until they are
    /// put in the same units as the screen. Doing that is what makes dragging a line move it by
    /// exactly as far as it was dragged rather than by however much of a share that came to.
    pub fn rescale(&mut self, size: Size) {
        let (split, parts, sizes) = match self {
            Self::Pane(_) => return,
            Self::Split {
                split,
                parts,
                sizes,
            } => (*split, parts, sizes),
        };

        let along: usize = match split {
            Split::Beside => size.columns,
            Split::Below => size.rows,
        };

        let lengths: Vec<usize> = Self::shares(along, sizes)
            .into_iter()
            .map(|(_, length)| length)
            .collect();
        *sizes = lengths.clone();

        for (part, length) in parts.iter_mut().zip(lengths) {
            let part_size: Size = match split {
                Split::Beside => Size::new(size.rows, length),
                Split::Below => Size::new(length, size.columns),
            };
            part.rescale(part_size);
        }
    }

    /// Move the line which is on the far side of the pane the given number of rows or columns,
    /// returning whether there was one there to move.
    ///
    /// What one part gains the next one along loses, so the rest of the window stays where it is.
    /// Neither of them is moved past having nothing left.
    ///
    /// The last part of an arrangement has nothing after it for a line to be between, so the line
    /// on the near side of it is moved instead. Which way it moves is the same either way, so
    /// moving a line right always moves it right whichever pane it was asked for.
    pub fn drag(&mut self, pane: &Uuid, split: Split, by: isize) -> bool {
        let (arranged, parts, sizes) = match self {
            Self::Pane(_) => return false,
            Self::Split {
                split,
                parts,
                sizes,
            } => (*split, parts, sizes),
        };

        // NOTE: The parts are tried before this arrangement of them is, so that the line which
        // moves is the one nearest the pane. A pane inside a split which is itself one part of a
        // bigger one is in both of them, and moving the line of the bigger one would move the
        // whole side of the window rather than the line which was taken hold of.
        if parts.iter_mut().any(|part| part.drag(pane, split, by)) {
            return true;
        }

        if arranged != split {
            return false;
        }

        // The pane is in one of these parts, which is the part the line is on the far side of. It
        // may be the part itself or inside it, since the line along the edge of a pane at the
        // edge of a split is the line of the split it is part of.
        let at: usize = match parts.iter().position(|part| part.panes().contains(pane)) {
            Some(at) => at,
            None => return false,
        };

        let (grows, shrinks) = match at + 1 < sizes.len() {
            true => (at, at + 1),
            false if at > 0 => (at - 1, at),
            false => return false,
        };

        let by: isize = by
            .max(1 - isize::try_from(sizes[grows]).unwrap_or(isize::MAX))
            .min(isize::try_from(sizes[shrinks]).unwrap_or(isize::MAX) - 1);

        sizes[grows] = sizes[grows].saturating_add_signed(by);
        sizes[shrinks] = sizes[shrinks].saturating_sub_signed(by);

        return true;
    }

    /// Put a new pane beside or below the given one, returning whether it was there to split.
    pub fn split(&mut self, pane: &Uuid, split: Split, new: Uuid) -> bool {
        match self {
            Self::Pane(this) if this == pane => {
                *self = Self::Split {
                    split,
                    parts: vec![Self::Pane(*this), Self::Pane(new)],
                    sizes: vec![Self::SHARE, Self::SHARE],
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split {
                split: arranged,
                parts,
                sizes,
            } => {
                // A pane split the same way as the ones it is among joins them rather than being
                // nested inside another split of its own.
                if *arranged == split {
                    if let Some(position) = parts
                        .iter()
                        .position(|part| matches!(part, Self::Pane(this) if this == pane))
                    {
                        // NOTE: The new pane takes half of what the one it was split off of had,
                        // and the line drawn between the two of them takes a row or a column out
                        // of that same pane rather than out of all of them. The sizes of the
                        // parts then still add up to what there is to share out, so none of the
                        // other lines in the window moves.
                        let had: usize = sizes[position].max(Self::DIVIDER + 2);
                        let left: usize = had - Self::DIVIDER;
                        let half: usize = (left / 2).max(1);

                        sizes[position] = left.saturating_sub(half).max(1);
                        sizes.insert(position + 1, half);

                        parts.insert(position + 1, Self::Pane(new));
                        return true;
                    }
                }

                parts.iter_mut().any(|part| part.split(pane, split, new))
            }
        }
    }

    /// Take the pane out, returning whether it was there to take out.
    ///
    /// A split which is left with only one part in it is not a split any more, so it is replaced
    /// by the part itself.
    ///
    /// NOTE: A pane which is the whole of the arrangement cannot be taken out of it, since there
    /// would be nothing left to be an arrangement of. Whatever owns the window has to drop the
    /// window instead, which is what [`Session::remove_pane`](crate::multiplexer) does.
    pub fn remove(&mut self, pane: &Uuid) -> bool {
        let (parts, sizes) = match self {
            Self::Pane(_) => return false,
            Self::Split { parts, sizes, .. } => (parts, sizes),
        };

        if let Some(position) = parts
            .iter()
            .position(|part| matches!(part, Self::Pane(this) if this == pane))
        {
            parts.remove(position);

            // What the pane had goes to whichever part is next to where it was, along with the
            // row or column the line beside it was taking, so that the rest of the window is not
            // left with a gap in it and none of the other lines moves.
            let had: usize = sizes.remove(position) + Self::DIVIDER;
            let beside: usize = position.min(sizes.len().saturating_sub(1));
            if let Some(size) = sizes.get_mut(beside) {
                *size += had;
            }
        } else if !parts.iter_mut().any(|part| part.remove(pane)) {
            return false;
        }

        if parts.len() == 1 {
            *self = parts.remove(0);
        }

        return true;
    }

    /// Swap the pane with the one before it in the window, or the one after it when that is which
    /// way is asked for, and return whether there was one to swap it with.
    ///
    /// Which pane is before or after it is worked out from the order every pane of the window is
    /// in rather than from the arrangement, so that this goes round all of them and not only the
    /// ones it happens to share a split with. Going past either end comes round to the other.
    ///
    /// The sizes stay where they are rather than going along with the panes, so swapping two of
    /// them moves what is in them about without the window changing shape.
    pub fn swap(&mut self, pane: &Uuid, after: bool) -> bool {
        let panes: Vec<Uuid> = self.panes();
        if panes.len() < 2 {
            return false;
        }

        let at: usize = match panes.iter().position(|this| this == pane) {
            Some(at) => at,
            None => return false,
        };

        let with: usize = match after {
            true => (at + 1) % panes.len(),
            false => (at + panes.len() - 1) % panes.len(),
        };

        self.exchange(pane, &panes[with]);
        return true;
    }

    /// Put each of the two panes where the other one was.
    ///
    /// NOTE: Every pane is in the arrangement once, so each of them is come across once and there
    /// is no putting one back where it started by going over it twice.
    fn exchange(&mut self, one: &Uuid, other: &Uuid) {
        match self {
            Self::Pane(pane) if pane == one => *pane = *other,
            Self::Pane(pane) if pane == other => *pane = *one,
            Self::Pane(_) => {}
            Self::Split { parts, .. } => {
                for part in parts {
                    part.exchange(one, other);
                }
            }
        }
    }

    /// Return the panes, in the order they are shown in.
    pub fn panes(&self) -> Vec<Uuid> {
        let mut panes: Vec<Uuid> = Vec::new();
        self.collect(&mut panes);
        return panes;
    }

    /// Put the panes onto the given ones, in the order they are shown in.
    fn collect(&self, panes: &mut Vec<Uuid>) {
        match self {
            Self::Pane(pane) => panes.push(*pane),
            Self::Split { parts, .. } => {
                for part in parts {
                    part.collect(panes);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return the pane with the given number, so that a test says which one it means.
    fn pane(number: u128) -> Uuid {
        return Uuid::from_u128(number);
    }

    /// Return where the top left corner of a window is.
    fn corner() -> Location {
        return Location::default();
    }

    /// Return where each pane of the tree is in a window of the given size, as the row, column,
    /// rows and columns of it.
    fn layout(tree: &Tree, size: Size) -> Vec<(usize, usize, usize, usize)> {
        return tree
            .layout(corner(), size)
            .into_iter()
            .map(|layout| {
                (
                    layout.at().row,
                    layout.at().column,
                    layout.size().rows,
                    layout.size().columns,
                )
            })
            .collect();
    }

    #[test]
    fn test_one_pane_is_given_the_whole_window() {
        let tree = Tree::Pane(pane(1));

        assert_eq!(layout(&tree, Size::new(10, 20)), vec![(0, 0, 10, 20)]);
    }

    #[test]
    fn test_two_panes_side_by_side_share_the_columns_with_a_line_between_them() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));

        // One column of the twenty goes to the line drawn between them.
        assert_eq!(
            layout(&tree, Size::new(10, 21)),
            vec![(0, 0, 10, 10), (0, 11, 10, 10)]
        );
    }

    #[test]
    fn test_two_panes_one_above_the_other_share_the_rows() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Below, pane(2));

        assert_eq!(
            layout(&tree, Size::new(11, 20)),
            vec![(0, 0, 5, 20), (6, 0, 5, 20)]
        );
    }

    #[test]
    fn test_what_is_left_over_goes_to_the_panes_at_the_start() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));

        // Nine columns for two panes and a line is four each with one left over.
        assert_eq!(
            layout(&tree, Size::new(4, 10)),
            vec![(0, 0, 4, 5), (0, 6, 4, 4)]
        );
    }

    #[test]
    fn test_splitting_the_same_way_again_shares_the_window_between_all_of_them() {
        let size = Size::new(4, 14);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.rescale(size);

        let first: (usize, usize, usize, usize) = layout(&tree, size)[0];

        tree.split(&pane(2), Split::Beside, pane(3));
        tree.rescale(size);

        // They are all beside each other rather than the third being nested inside the second.
        assert_eq!(tree.panes(), vec![pane(1), pane(2), pane(3)]);
        // Splitting the second one halves that one, so the first is exactly where it was and the
        // two which came out of the second share what it had.
        let laid_out = layout(&tree, size);
        assert_eq!(laid_out[0], first);
        assert_eq!(laid_out, vec![(0, 0, 4, 7), (0, 8, 4, 3), (0, 12, 4, 2)]);
    }

    #[test]
    fn test_splitting_the_other_way_nests_a_split_inside() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Below, pane(3));

        assert_eq!(
            layout(&tree, Size::new(5, 21)),
            vec![(0, 0, 5, 10), (0, 11, 2, 10), (3, 11, 2, 10)]
        );
    }

    #[test_case(&[2], &[1, 3]; "one of three, which leaves a split of two")]
    #[test_case(&[2, 3], &[1]; "all but one, which leaves no split at all")]
    fn test_a_split_with_one_part_left_is_not_a_split_any_more(removed: &[u128], left: &[u128]) {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Beside, pane(3));

        for number in removed {
            tree.remove(&pane(*number));
        }

        assert_eq!(
            tree.panes(),
            left.iter().map(|n| pane(*n)).collect::<Vec<Uuid>>()
        );
        assert_eq!(matches!(tree, Tree::Pane(_)), left.len() == 1);
    }

    /// Return a window of two panes side by side in a screen of the given size, with the sizes of
    /// the parts already put in the same units as the screen the way a session does.
    fn beside(size: Size) -> Tree {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.rescale(size);
        return tree;
    }

    #[test]
    fn test_dragging_the_line_between_two_panes_moves_it() {
        let size = Size::new(4, 21);
        let mut tree = beside(size);

        // The line moves two columns to the right, so the pane on the left gains them and the one
        // on the right loses them.
        assert_eq!(tree.drag(&pane(1), Split::Beside, 2), true);

        assert_eq!(layout(&tree, size), vec![(0, 0, 4, 12), (0, 13, 4, 8)]);
    }

    #[test]
    fn test_dragging_the_line_the_other_way_moves_it_back() {
        let size = Size::new(4, 21);
        let mut tree = beside(size);
        tree.drag(&pane(1), Split::Beside, 2);
        tree.rescale(size);

        tree.drag(&pane(1), Split::Beside, -2);

        assert_eq!(layout(&tree, size), vec![(0, 0, 4, 10), (0, 11, 4, 10)]);
    }

    #[test]
    fn test_a_pane_is_never_dragged_past_having_nothing_left() {
        let size = Size::new(4, 21);
        let mut tree = beside(size);

        // Far more than there is to give.
        tree.drag(&pane(1), Split::Beside, 100_000);

        for (_, _, rows, columns) in layout(&tree, size) {
            assert!(rows >= 1);
            assert!(columns >= 1);
        }
    }

    #[test]
    fn test_the_panes_keep_how_they_were_left_when_the_window_changes_size() {
        let mut tree = beside(Size::new(4, 21));
        tree.drag(&pane(1), Split::Beside, 5);

        // Two thirds of the window and one third of it, whatever size it is.
        assert_eq!(
            layout(&tree, Size::new(4, 21)),
            vec![(0, 0, 4, 15), (0, 16, 4, 5)]
        );
        assert_eq!(
            layout(&tree, Size::new(4, 41)),
            vec![(0, 0, 4, 30), (0, 31, 4, 10)]
        );
    }

    #[test]
    fn test_the_line_on_the_near_side_of_the_last_pane_is_the_one_which_moves() {
        let size = Size::new(4, 21);
        let mut tree = beside(size);

        // Nothing is beside the second pane, so the line between it and the first one moves, and
        // it moves the same way it would have for the first pane.
        assert_eq!(tree.drag(&pane(2), Split::Beside, 2), true);

        assert_eq!(layout(&tree, size), vec![(0, 0, 4, 12), (0, 13, 4, 8)]);
    }

    /// Return where each line down the window is, which is the column just past each pane but
    /// the last.
    fn lines(tree: &Tree, size: Size) -> Vec<usize> {
        let mut at: Vec<usize> = tree
            .layout(corner(), size)
            .iter()
            .map(|pane| pane.at().column + pane.size().columns)
            .collect();
        at.sort_unstable();
        at.dedup();
        at.pop();
        return at;
    }

    #[test]
    fn test_splitting_a_pane_does_not_move_the_lines_beside_the_others() {
        let size = Size::new(9, 80);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Beside, pane(3));
        tree.rescale(size);

        let before: Vec<usize> = lines(&tree, size);

        // The first pane is split, so only the lines within what it had may move.
        tree.split(&pane(1), Split::Beside, pane(4));
        tree.rescale(size);
        let after: Vec<usize> = lines(&tree, size);

        // The lines which were already there are still where they were, with one more among them.
        assert_eq!(after.len(), before.len() + 1);
        for line in &before {
            assert!(after.contains(line), "line at {} moved: {:?}", line, after);
        }
    }

    #[test]
    fn test_closing_a_pane_does_not_move_the_lines_beside_the_others() {
        let size = Size::new(9, 80);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Beside, pane(3));
        tree.split(&pane(3), Split::Beside, pane(4));
        tree.rescale(size);

        let before: Vec<usize> = lines(&tree, size);
        tree.remove(&pane(4));
        tree.rescale(size);
        let after: Vec<usize> = lines(&tree, size);

        // The last pane went, so the line before it went with it and the rest stayed put.
        assert_eq!(after, before[..before.len() - 1].to_vec());
    }

    #[test]
    fn test_the_line_nearest_the_pane_is_the_one_which_moves() {
        // One pane on the left and two stacked on the right, with the lower of those split again
        // so that there is a line down the middle of it.
        let size = Size::new(9, 41);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Below, pane(3));
        tree.split(&pane(3), Split::Beside, pane(4));
        tree.rescale(size);

        let before: Vec<(usize, usize, usize, usize)> = layout(&tree, size);
        assert_eq!(tree.drag(&pane(3), Split::Beside, 2), true);
        let after: Vec<(usize, usize, usize, usize)> = layout(&tree, size);

        // The line down the middle of the lower right pair moved, and the one down the middle of
        // the whole window stayed where it was.
        assert_eq!(after[0], before[0]);
        assert_ne!(after[2], before[2]);
    }

    #[test]
    fn test_a_pane_inside_a_split_moves_the_line_of_the_one_it_is_part_of() {
        // The pane at the top right has no line of its own running down the screen, so the one
        // which moves is the line down the middle of the whole window.
        let size = Size::new(9, 41);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Below, pane(3));
        tree.rescale(size);

        let before: Vec<(usize, usize, usize, usize)> = layout(&tree, size);
        assert_eq!(tree.drag(&pane(2), Split::Beside, 2), true);
        let after: Vec<(usize, usize, usize, usize)> = layout(&tree, size);

        assert_ne!(after[0], before[0]);
    }

    #[test]
    fn test_splitting_one_above_the_other_shares_the_rows_evenly() {
        // Twenty three rows is a line and eleven rows each, with none left over.
        let size = Size::new(23, 40);
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Below, pane(2));
        tree.rescale(size);

        let laid_out = layout(&tree, size);
        assert_eq!(laid_out[0].2, laid_out[1].2);
    }

    #[test]
    fn test_there_is_no_line_to_drag_which_runs_the_other_way() {
        let mut tree = beside(Size::new(4, 21));

        // The line between the panes runs down the screen rather than across it.
        assert_eq!(tree.drag(&pane(1), Split::Below, 2), false);
    }

    #[test]
    fn test_what_a_pane_which_goes_had_is_given_to_the_one_next_to_it() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Beside, pane(3));

        tree.remove(&pane(3));

        // The third had a quarter of the window and the second now has it as well as its own.
        assert_eq!(
            layout(&tree, Size::new(4, 21)),
            vec![(0, 0, 4, 10), (0, 11, 4, 10)]
        );
    }

    #[test]
    fn test_a_pane_which_is_the_whole_arrangement_cannot_be_taken_out_of_it() {
        let mut tree = Tree::Pane(pane(1));

        // There would be nothing left to be an arrangement of, so the window goes instead.
        assert_eq!(tree.remove(&pane(1)), false);
        assert_eq!(tree.panes(), vec![pane(1)]);
    }

    #[test]
    fn test_a_pane_is_swapped_with_the_one_after_it() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));

        assert_eq!(tree.swap(&pane(1), true), true);

        assert_eq!(tree.panes(), vec![pane(2), pane(1)]);
    }

    #[test]
    fn test_a_pane_is_swapped_with_the_one_before_it() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));

        assert_eq!(tree.swap(&pane(2), false), true);

        assert_eq!(tree.panes(), vec![pane(2), pane(1)]);
    }

    #[test]
    fn test_the_sizes_stay_where_they_are_when_two_panes_are_swapped() {
        let size = Size::new(4, 21);
        let mut tree = beside(size);
        tree.drag(&pane(1), Split::Beside, 4);

        tree.swap(&pane(1), true);

        // The one on the left is still the wider of the two, whichever pane is in it now.
        assert_eq!(layout(&tree, size), vec![(0, 0, 4, 14), (0, 15, 4, 6)]);
        assert_eq!(tree.panes(), vec![pane(2), pane(1)]);
    }

    #[test]
    fn test_swapping_goes_round_the_end_to_the_other() {
        let mut tree = beside(Size::new(4, 21));

        assert_eq!(tree.swap(&pane(1), false), true);

        assert_eq!(tree.panes(), vec![pane(2), pane(1)]);
    }

    #[test]
    fn test_swapping_goes_round_every_pane_of_the_window() {
        // One pane on the left and two stacked on the right, so the panes are in more than one
        // split and going round them all means leaving the one a pane is in.
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Below, pane(3));
        assert_eq!(tree.panes(), vec![pane(1), pane(2), pane(3)]);

        // The pane on the left has no pane beside it in its own split, and the one after it is
        // the one at the top right.
        assert_eq!(tree.swap(&pane(1), true), true);

        assert_eq!(tree.panes(), vec![pane(2), pane(1), pane(3)]);
    }

    #[test]
    fn test_a_pane_which_is_the_only_one_has_nothing_to_swap_with() {
        let mut tree = Tree::Pane(pane(1));

        assert_eq!(tree.swap(&pane(1), true), false);
    }

    #[test]
    fn test_a_pane_which_is_not_there_is_not_split_or_taken_out() {
        let mut tree = Tree::Pane(pane(1));

        assert_eq!(tree.split(&pane(9), Split::Beside, pane(2)), false);
        assert_eq!(tree.remove(&pane(9)), false);
        assert_eq!(tree.panes(), vec![pane(1)]);
    }

    #[test]
    fn test_a_pane_is_never_given_no_room_at_all() {
        let mut tree = Tree::Pane(pane(1));
        tree.split(&pane(1), Split::Beside, pane(2));
        tree.split(&pane(2), Split::Beside, pane(3));

        // There is not enough room to go around, so they are given the least there is rather than
        // nothing at all.
        for (_, _, rows, columns) in layout(&tree, Size::new(1, 2)) {
            assert!(rows >= 1);
            assert!(columns >= 1);
        }
    }
}
