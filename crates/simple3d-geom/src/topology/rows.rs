//! A table of variable-length rows in two flat arrays, like a sparse matrix.

/// Rows of items: row `r` is `items[start[r]..start[r + 1]]`. Two allocations for any number of
/// rows, where a `Vec` per row cost one each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rows<T> {
    start: Vec<u32>,
    items: Vec<T>,
}

impl<T> Default for Rows<T> {
    fn default() -> Rows<T> {
        Rows { start: vec![0], items: Vec::new() }
    }
}

/// Fills [`Rows`] in two passes over the same pairs: [`count`](RowsBuilder::count) every
/// (row, item) pair, then [`push`](RowsBuilder::push) them all in the same order. Plain loops at the
/// caller rather than a closure yielding the pairs, which doubled the edge table's time.
pub struct RowsBuilder<T> {
    start: Vec<u32>,
    cursor: Vec<u32>,
    items: Vec<T>,
}

impl<T: Copy + Default> RowsBuilder<T> {
    pub fn new(rows: usize) -> RowsBuilder<T> {
        RowsBuilder { start: vec![0; rows + 1], cursor: Vec::new(), items: Vec::new() }
    }

    /// Count one item into `row`, in the first pass.
    #[inline]
    pub fn count(&mut self, row: usize) {
        self.start[row + 1] += 1;
    }

    /// Put `item` next in `row`, in the second pass. The first call ends the counting.
    #[inline]
    pub fn push(&mut self, row: usize, item: T) {
        if self.cursor.is_empty() {
            self.lay_out();
        }
        self.items[self.cursor[row] as usize] = item;
        self.cursor[row] += 1;
    }

    fn lay_out(&mut self) {
        for r in 1..self.start.len() {
            self.start[r] += self.start[r - 1];
        }
        self.cursor = self.start.clone();
        self.items = vec![T::default(); *self.start.last().expect("one boundary per row and one more") as usize];
    }

    /// The rows, each in the order its items were pushed. Every counted item must have been pushed.
    pub fn finish(mut self) -> Rows<T> {
        if self.cursor.is_empty() {
            self.lay_out();
        }
        debug_assert!(self.cursor[..self.cursor.len() - 1] == self.start[1..], "counted more than was pushed");
        Rows { start: self.start, items: self.items }
    }
}

impl<T> Rows<T> {
    /// Rows from their boundaries and items, as built by hand; `start` begins at 0 and ends at
    /// `items.len()`.
    pub(crate) fn from_parts(start: Vec<u32>, items: Vec<T>) -> Rows<T> {
        debug_assert!(start.first() == Some(&0) && start.last().map(|&s| s as usize) == Some(items.len()));
        Rows { start, items }
    }

    /// How many rows there are.
    pub fn len(&self) -> usize {
        self.start.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn row(&self, r: usize) -> &[T] {
        &self.items[self.start[r] as usize..self.start[r + 1] as usize]
    }

    pub(crate) fn row_mut(&mut self, r: usize) -> &mut [T] {
        &mut self.items[self.start[r] as usize..self.start[r + 1] as usize]
    }

    /// Every item, row after row.
    pub fn items(&self) -> &[T] {
        &self.items
    }
}
