//! The cells a straight line crosses, one step at a time, as High Five servers walk them.

/// Steps from one cell to another, moving one cell along the longer axis at a time and the shorter one as the
/// line calls for it; the cell it starts on is not given back.
pub(crate) struct Line {
    at: [i32; 2],
    to: [i32; 2],
    /// How far the line runs on each axis, and which way it goes.
    run: [i32; 2],
    toward: [i32; 2],
    error: i32,
}

impl Line {
    pub(crate) fn new(from: [i32; 2], to: [i32; 2]) -> Self {
        let run = [(to[0] - from[0]).abs(), (to[1] - from[1]).abs()];
        let toward = [if from[0] < to[0] { 1 } else { -1 }, if from[1] < to[1] { 1 } else { -1 }];
        let error = run[0].max(run[1]) / 2;
        Self { at: from, to, run, toward, error }
    }
}

impl Iterator for Line {
    type Item = [i32; 2];

    fn next(&mut self) -> Option<[i32; 2]> {
        let [along, across] = if self.run.first() >= self.run.get(1) { [0, 1] } else { [1, 0] };
        let (run_along, run_across) = (*self.run.get(along)?, *self.run.get(across)?);
        if self.at.get(along) == self.to.get(along) {
            return None;
        }
        *self.at.get_mut(along)? += *self.toward.get(along)?;
        self.error += run_across;
        if self.error >= run_along {
            *self.at.get_mut(across)? += *self.toward.get(across)?;
            self.error -= run_along;
        }
        Some(self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_steps_cell_by_cell_to_where_it_ends() {
        let steps: Vec<[i32; 2]> = Line::new([0, 0], [3, 0]).collect();
        assert_eq!(steps, [[1, 0], [2, 0], [3, 0]], "a straight line takes one cell a step");
        let steps: Vec<[i32; 2]> = Line::new([0, 0], [3, 3]).collect();
        assert_eq!(steps, [[1, 1], [2, 2], [3, 3]], "a line at a right angle takes the diagonal");
        let steps: Vec<[i32; 2]> = Line::new([0, 0], [-4, 2]).collect();
        assert_eq!(steps.last(), Some(&[-4, 2]), "a line the other way still arrives");
        assert_eq!(steps.len(), 4, "and takes as many steps as its longer axis");
        assert_eq!(Line::new([5, 5], [5, 5]).count(), 0, "a line that goes nowhere takes no step");
    }
}
