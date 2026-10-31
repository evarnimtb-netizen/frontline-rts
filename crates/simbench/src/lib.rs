//! Frontline M0 spike benchmarks (§2.2).
//!
//! The library holds the benchmark kernels so they can be unit-tested; the
//! `simbench` binary (`src/main.rs`) is the command-line front end.
//!
//! * [`grid`]: a 1 m heightfield in three memory layouts (row-major, 16×16
//!   tiles, Morton/Z-order) with a max-mip pyramid.
//! * [`los`]: an integer DDA line-of-sight march over that heightfield, with
//!   and without max-mip block skipping.
//! * [`orca`]: a standalone fixed-point ORCA avoidance kernel with a 16 m
//!   spatial hash (mirrors `frontline_sim`'s movement stage).
//! * [`tick`]: the composed tick, i.e. `frontline_sim::World::step` at tier caps plus a
//!   synthetic per-tick ray budget.
//!
//! All kernel arithmetic is integer / Q16.16. Floats appear only in timing
//! and reporting.

pub mod grid;
pub mod los;
pub mod orca;
pub mod report;
pub mod stats;
pub mod tick;
