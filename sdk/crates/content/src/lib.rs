//! Def and scenario types for Frontline content: TOML in, JSON Schema out.
//!
//! Everything here is plain data plus validation. The crate does not depend on
//! the simulation; tools such as `frontline-cli` lower these types into sim
//! configuration themselves. Validation collects every problem it finds and
//! reports each as an [`Issue`] with a dotted field path and, where possible, a
//! concrete fix, so a human or an agent can repair a file in one pass (§8.2).
//!
//! Content values are integers only (metres, milliseconds, hertz), so lowering
//! them into the fixed-point sim never depends on float rounding.

mod issue;
pub mod scenario;

pub use issue::{ContentError, Issue};
pub use scenario::{
    ForcesDef, HeightfieldDef, IntervalMs, MapDef, ProjectilesDef, Scenario, TICK_HZ_ALLOWED,
};

/// One exported JSON Schema.
pub struct SchemaDoc {
    /// Def type name; the file is written as `<name>.schema.json`.
    pub name: &'static str,
    pub schema: schemars::Schema,
}

/// JSON Schemas for every def type, in a stable order.
pub fn schemas() -> Vec<SchemaDoc> {
    vec![SchemaDoc {
        name: "scenario",
        schema: schemars::schema_for!(Scenario),
    }]
}
