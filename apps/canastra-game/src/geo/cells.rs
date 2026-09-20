//! One map tile of geodata, as High Five stores it: blocks of cells, each with a height and the sides a
//! character may leave it by.
//!
//! A region file is 256 by 256 blocks in x-major order, each block 8 by 8 cells, each cell 16 world units
//! square. A block starts with its kind and holds one height for all its cells (flat), one per cell
//! (complex), or a stack of layers per cell (multilayer). A cell's two bytes pack the height in the top
//! twelve bits, halved, and the sides it opens onto in the low four.

/// World units a cell covers on X and Y.
pub(crate) const CELL: i32 = 16;
/// Cells a block covers on each axis, and blocks a region covers on each axis.
const BLOCK_CELLS: usize = 8;
const REGION_BLOCKS: usize = 256;
/// Cells a region covers on each axis: one map tile, 32768 world units.
pub(crate) const REGION_CELLS: i32 = 256 * 8;

/// The sides a character may leave a cell by, as the low four bits of a cell.
pub(crate) const EAST: u8 = 1;
pub(crate) const WEST: u8 = 1 << 1;
pub(crate) const SOUTH: u8 = 1 << 2;
pub(crate) const NORTH: u8 = 1 << 3;

const FLAT: u8 = 0;
const COMPLEX: u8 = 1;
const MULTILAYER: u8 = 2;

/// A region file, kept as it was read, with where each of its blocks begins.
pub(crate) struct Region {
    bytes: Vec<u8>,
    /// Where each block starts, by block, x-major.
    blocks: Vec<u32>,
}

/// What a cell says: how high its floor is and which sides it opens onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    pub(crate) height: i32,
    pub(crate) sides: u8,
}

impl Region {
    /// Reads a region file, or says where it stops making sense.
    pub(crate) fn read(bytes: Vec<u8>) -> Result<Self, String> {
        let mut blocks = Vec::with_capacity(REGION_BLOCKS * REGION_BLOCKS);
        let mut at = 0usize;
        for _ in 0..REGION_BLOCKS * REGION_BLOCKS {
            let start = u32::try_from(at).map_err(|_| "a region file is too long".to_owned())?;
            blocks.push(start);
            let kind = *bytes.get(at).ok_or_else(|| format!("a region file ends inside block {}", blocks.len()))?;
            at += 1;
            at += match kind {
                FLAT => 2,
                COMPLEX => BLOCK_CELLS * BLOCK_CELLS * 2,
                MULTILAYER => {
                    let mut wide = 0;
                    for _ in 0..BLOCK_CELLS * BLOCK_CELLS {
                        let layers = usize::from(*bytes.get(at + wide).ok_or("a region file ends inside a cell")?);
                        wide += 1 + layers * 2;
                    }
                    wide
                }
                other => return Err(format!("a region file holds block kind {other}")),
            };
        }
        Ok(Self { bytes, blocks })
    }

    /// The floors of the cell at `[x, y]` within this region, lowest first; a place outdoors has one.
    pub(crate) fn floors(&self, cell: [usize; 2]) -> Vec<Cell> {
        self.read_floors(cell).unwrap_or_default()
    }

    fn read_floors(&self, [x, y]: [usize; 2]) -> Option<Vec<Cell>> {
        let (block_x, block_y) = (x / BLOCK_CELLS, y / BLOCK_CELLS);
        let at = usize::try_from(*self.blocks.get(block_x * REGION_BLOCKS + block_y)?).ok()?;
        let kind = *self.bytes.get(at)?;
        let within = (x % BLOCK_CELLS) * BLOCK_CELLS + (y % BLOCK_CELLS);
        match kind {
            FLAT => Some(vec![Cell { height: i32::from(self.short(at + 1)?), sides: NORTH | SOUTH | EAST | WEST }]),
            COMPLEX => Some(vec![cell_of(self.short(at + 1 + within * 2)?)]),
            MULTILAYER => {
                // Cells keep their layers one after another, so the ones before this cell are stepped over.
                let mut layers_at = at + 1;
                for _ in 0..within {
                    layers_at += 1 + usize::from(*self.bytes.get(layers_at)?) * 2;
                }
                let layers = usize::from(*self.bytes.get(layers_at)?);
                let mut floors: Vec<Cell> =
                    (0..layers).filter_map(|layer| Some(cell_of(self.short(layers_at + 1 + layer * 2)?))).collect();
                floors.sort_by_key(|cell| cell.height);
                Some(floors)
            }
            _ => None,
        }
    }

    fn short(&self, at: usize) -> Option<i16> {
        let bytes = self.bytes.get(at..at + 2)?;
        Some(i16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]))
    }
}

/// What a cell's two bytes hold: the height in the top twelve bits, stored halved, and the open sides in the
/// low four.
fn cell_of(stored: i16) -> Cell {
    let sides = u8::try_from(stored.cast_unsigned() & 0x000F).unwrap_or_default();
    Cell { height: i32::from(stored & -16i16) / 2, sides }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A region of one flat block and the rest complex, with a known height in the second block's first cell.
    fn region() -> Region {
        let mut bytes = vec![FLAT, 0x10, 0x00];
        for block in 1..REGION_BLOCKS * REGION_BLOCKS {
            bytes.push(COMPLEX);
            for cell in 0..BLOCK_CELLS * BLOCK_CELLS {
                // A height of -1000 is stored doubled in the top twelve bits, with the open sides under it.
                let deep = (-2000i16).cast_unsigned() & 0xFFF0;
                let stored = if block == 1 && cell == 0 { (deep | u16::from(NORTH | EAST)).cast_signed() } else { 0 };
                bytes.extend(stored.to_le_bytes());
            }
        }
        Region::read(bytes).expect("the region is well formed")
    }

    #[test]
    fn cells_carry_their_height_and_the_sides_they_open_onto() {
        let region = region();
        let flat = region.floors([0, 0]);
        assert_eq!(flat, [Cell { height: 16, sides: NORTH | SOUTH | EAST | WEST }], "a flat block opens every side");
        // The second block covers cells 8 to 15 on Y, since blocks run along Y first.
        assert_eq!(region.floors([0, 8]), [Cell { height: -1000, sides: NORTH | EAST }]);
        assert_eq!(region.floors([0, 9]).first().map(|cell| cell.height), Some(0), "other cells keep their own");
    }

    #[test]
    fn a_short_file_is_refused_rather_than_read_wrong() {
        assert!(Region::read(vec![FLAT, 0x10]).is_err());
        assert!(Region::read(vec![9, 0, 0]).is_err(), "an unknown block kind is refused");
    }
}
