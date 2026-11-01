//! Uniform spatial hash (§5.4): 16 m cells over the square map, rebuilt every
//! tick in O(n) by a counting sort over ascending `SimId`, so each cell lists
//! its entities in id order. Stored as a dense grid in CSR form
//! (`cell_start` offsets into `ids`).
//!
//! The hash is a pure function of entity positions. It is not saved or
//! hashed; it is rebuilt after load.

use crate::ecs::{Entities, EntityKind, SimId};
use crate::fx::{Fx, FxVec2};

pub const CELL_M: i32 = 16;

#[derive(Clone, Debug, Default)]
pub struct SpatialHash {
    dim: u32,
    /// `cell_start[c]..cell_start[c + 1]` indexes `ids` for cell `c`.
    cell_start: Vec<u32>,
    ids: Vec<SimId>,
    /// Scratch: cell of each id (u32::MAX if not indexed).
    cell_of_id: Vec<u32>,
    /// Scratch: fill cursor per cell.
    cursor: Vec<u32>,
}

impl SpatialHash {
    pub fn new(map_size_m: i32) -> SpatialHash {
        let dim = (map_size_m / CELL_M).max(1) as u32;
        SpatialHash {
            dim,
            cell_start: vec![0; (dim * dim + 1) as usize],
            ids: Vec::new(),
            cell_of_id: Vec::new(),
            cursor: Vec::new(),
        }
    }

    #[inline]
    fn cell_coord(&self, v: Fx) -> i32 {
        (v.raw() >> Fx::FRAC_BITS)
            .div_euclid(CELL_M)
            .clamp(0, self.dim as i32 - 1)
    }

    #[inline]
    pub fn cell_index(&self, p: FxVec2) -> u32 {
        self.cell_coord(p.y) as u32 * self.dim + self.cell_coord(p.x) as u32
    }

    /// Rebuilds from all live soldiers.
    pub fn rebuild(&mut self, ents: &Entities) {
        self.rebuild_kinds(ents, |k| k == EntityKind::Soldier);
    }

    /// Rebuilds from the live entities whose kind passes `keep` (S3: the
    /// vehicle index holds vehicles and wrecks at their hull origins).
    pub fn rebuild_kinds(&mut self, ents: &Entities, keep: impl Fn(EntityKind) -> bool) {
        let n_cells = (self.dim * self.dim) as usize;
        self.cell_start.clear();
        self.cell_start.resize(n_cells + 1, 0);
        self.cell_of_id.clear();
        self.cell_of_id.resize(ents.len(), u32::MAX);
        for (i, kind) in ents.kind.iter().enumerate() {
            if keep(*kind) {
                let c = self.cell_index(ents.transform[i].pos.xy());
                self.cell_of_id[i] = c;
                self.cell_start[c as usize + 1] += 1;
            }
        }
        for c in 0..n_cells {
            self.cell_start[c + 1] += self.cell_start[c];
        }
        let total = self.cell_start[n_cells] as usize;
        self.ids.clear();
        self.ids.resize(total, SimId(0));
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.cell_start);
        for (i, &c) in self.cell_of_id.iter().enumerate() {
            if c != u32::MAX {
                let slot = &mut self.cursor[c as usize];
                self.ids[*slot as usize] = SimId(i as u32);
                *slot += 1;
            }
        }
    }

    /// Ids in cells overlapping the square `center ± radius`, cell by cell in
    /// row-major order, each cell in ascending id.
    pub fn for_each_near(&self, center: FxVec2, radius: Fx, mut f: impl FnMut(SimId)) {
        let x0 = self.cell_coord(center.x - radius);
        let x1 = self.cell_coord(center.x + radius);
        let y0 = self.cell_coord(center.y - radius);
        let y1 = self.cell_coord(center.y + radius);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let c = (cy as u32 * self.dim + cx as u32) as usize;
                for &id in &self.ids[self.cell_start[c] as usize..self.cell_start[c + 1] as usize] {
                    f(id);
                }
            }
        }
    }
}
