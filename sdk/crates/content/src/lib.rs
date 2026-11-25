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
//! * [`resolve`] (S4): `extends` chains over raw def tables.
//! * [`variant`] (S4): `[[members]]`, capabilities, tags, training
//!   profiles, and the legacy squad expansion.
//! * [`tuning`]: gameplay tuning tables of the rules (S3b), shared by the
//!   def and the setup.
//!
//! Content values are integers only (metres, milliseconds, hertz), so lowering
//! them into the fixed-point sim never depends on float rounding.

pub mod defs;
mod issue;
pub mod modset;
pub mod resolve;
pub mod scenario;
pub mod setup;
pub mod tuning;
pub mod variant;
pub mod vehicle;

pub use defs::{
    CartridgeDef, FactionDef, ITEM_KINDS, ItemDef, ItemKind, MAX_COOLDOWN_S,
    MAX_INITIAL_COOLDOWN_S, MapFileDef, RulesDef, SquadDef, SquadRole, SupplyDef, SupplyKind,
    WeaponClass, WeaponDef,
};
pub use issue::{ContentError, Issue};
pub use modset::{ModDefs, ModManifest, OverrideChange};
pub use resolve::{MAX_EXTENDS_DEPTH, RawDef, RawDefs};
pub use scenario::{
    ForcesDef, HeightfieldDef, IntervalMs, MapDef, ProjectilesDef, Scenario, TICK_HZ_ALLOWED,
};
pub use setup::{
    Difficulty, DigProfile, Fixed, FixedVec2, MatchRequest, MatchSetup, PlayerRequest, PlayerSetup,
};
pub use variant::{MemberDef, TrainingDef};
pub use vehicle::{
    AmmoDef, AmmoMechanism, COMPONENTS, ComponentKind, Frame, MAX_ARMOUR_BOXES, MAX_ZONES,
    MobilityKind, MountKind, PLATE_GROUPS, PlateGroup, SEAT_ROLES, SeatRole, SlopeClass,
    VehicleDef, VehicleRole,
};

/// Most capture points on a map (the sim's AI keeps point indices in a
/// byte with one value reserved).
pub const MAX_CAPTURE_POINTS: usize = 64;
/// S16a: most roads on a map, and most points per road.
pub const MAX_ROADS: usize = 64;
pub const MAX_ROAD_POINTS: usize = 64;

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
            name: "training",
            schema: schemars::schema_for!(TrainingDef),
        },
        SchemaDoc {
            name: "cartridge",
            schema: schemars::schema_for!(crate::defs::CartridgeDef),
        },
        SchemaDoc {
            name: "supply",
            schema: schemars::schema_for!(crate::defs::StockDef),
        },
        SchemaDoc {
            name: "match_request",
            schema: schemars::schema_for!(MatchRequest),
        },
    ]
    .into_iter()
    .map(|mut d| {
        if DEF_SCHEMAS.contains(&d.name) {
            add_extends(&mut d.schema);
        }
        d
    })
    .collect()
}

/// Schemas of def files, which may name a parent with `extends` (S4).
const DEF_SCHEMAS: [&str; 11] = [
    "squad",
    "weapon",
    "item",
    "vehicle",
    "ammo",
    "faction",
    "map",
    "rules",
    "training",
    "cartridge",
    "supply",
];

/// Adds the S4 `extends` key (resolved away by the loader, so not a field
/// of the def structs) to a def schema. A file with `extends` needs only
/// its `id`: the other required keys apply to files without it.
fn add_extends(schema: &mut schemars::Schema) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    if let Some(req) = obj.remove("required") {
        let mut id_only = req.clone();
        if let Some(a) = id_only.as_array_mut() {
            a.retain(|v| v.as_str() == Some("id"));
        }
        obj.insert("required".into(), id_only);
        obj.insert(
            "if".into(),
            schemars::json_schema!({ "not": { "required": ["extends"] } }).to_value(),
        );
        let mut then = schemars::json_schema!({});
        then.insert("required".into(), req);
        obj.insert("then".into(), then.to_value());
    }
    let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) else {
        return;
    };
    props.insert(
        "extends".into(),
        schemars::json_schema!({
            "description": "S4: id of a def of the same kind this file starts from; \
                tables merge key by key, arrays and values replace (at most 4 parents; \
                docs/modding/variants.md).",
            "type": "string"
        })
        .to_value(),
    );
}
