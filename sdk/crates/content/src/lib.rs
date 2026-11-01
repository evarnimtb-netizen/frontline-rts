//! Def and scenario types for Frontline content: TOML in, JSON Schema out.
//!
//! Everything here is plain data plus validation. The crate does not depend on
//! the simulation (it is part of the open SDK, §7.9); the sim depends on it
//! and runs the [`MatchSetup`] this crate resolves. Validation collects every
//! problem it finds and reports each as an [`Issue`] with a dotted field path
//! and, where possible, a concrete fix, so a human or an agent can repair a
//! file in one pass (§8.2).
//!
//! * [`scenario`]: headless benchmark battles (`frontline-cli simulate`).
//! * [`defs`] + [`modset`]: a content mod's def files (squads, weapons,
//!   items, factions, maps, rules), loaded with [`ModDefs::load`].
//! * [`setup`]: [`ModDefs::resolve`] turns defs plus a [`MatchRequest`] into a
//!   fixed-point [`MatchSetup`] for `frontline_sim::World::new_match`.
//! * [`vehicle`]: vehicle and ammo defs (S3) and their setups.
//! * [`tuning`]: gameplay tuning tables of the rules (S3b), shared by the
//!   def and the setup.
//!
//! Content values are integers only (metres, milliseconds, hertz), so lowering
//! them into the fixed-point sim never depends on float rounding.

pub mod defs;
mod issue;
pub mod modset;
pub mod scenario;
pub mod setup;
pub mod tuning;
pub mod vehicle;

pub use defs::{
    FactionDef, ITEM_KINDS, ItemDef, ItemKind, MAX_COOLDOWN_S, MAX_INITIAL_COOLDOWN_S, MapFileDef,
    RulesDef, SquadDef, SquadRole, WeaponClass, WeaponDef,
};
pub use issue::{ContentError, Issue};
pub use modset::{ModDefs, ModManifest, OverrideChange};
pub use scenario::{
    ForcesDef, HeightfieldDef, IntervalMs, MapDef, ProjectilesDef, Scenario, TICK_HZ_ALLOWED,
};
pub use setup::{
    Difficulty, DigProfile, Fixed, FixedVec2, MatchRequest, MatchSetup, PlayerRequest, PlayerSetup,
};
pub use vehicle::{
    ARMOUR_ZONES, AmmoDef, AmmoMechanism, ArmourZone, COMPONENTS, ComponentKind, Frame, MountKind,
    SEAT_ROLES, SeatRole, VehicleDef,
};

/// Most capture points on a map (the sim's AI keeps point indices in a
/// byte with one value reserved).
pub const MAX_CAPTURE_POINTS: usize = 64;

/// One exported JSON Schema.
pub struct SchemaDoc {
    /// Def type name; the file is written as `<name>.schema.json`.
    pub name: &'static str,
    pub schema: schemars::Schema,
}

/// JSON Schemas for every def type, in a stable order.
pub fn schemas() -> Vec<SchemaDoc> {
    vec![
        SchemaDoc {
            name: "scenario",
            schema: schemars::schema_for!(Scenario),
        },
        SchemaDoc {
            name: "mod",
            schema: schemars::schema_for!(ModManifest),
        },
        SchemaDoc {
            name: "squad",
            schema: schemars::schema_for!(SquadDef),
        },
        SchemaDoc {
            name: "weapon",
            schema: schemars::schema_for!(WeaponDef),
        },
        SchemaDoc {
            name: "item",
            schema: schemars::schema_for!(ItemDef),
        },
        SchemaDoc {
            name: "vehicle",
            schema: schemars::schema_for!(VehicleDef),
        },
        SchemaDoc {
            name: "ammo",
            schema: schemars::schema_for!(AmmoDef),
        },
        SchemaDoc {
            name: "faction",
            schema: schemars::schema_for!(FactionDef),
        },
        SchemaDoc {
            name: "map",
            schema: schemars::schema_for!(MapFileDef),
        },
        SchemaDoc {
            name: "rules",
            schema: schemars::schema_for!(RulesDef),
        },
        SchemaDoc {
            name: "match_request",
            schema: schemars::schema_for!(MatchRequest),
        },
    ]
}
