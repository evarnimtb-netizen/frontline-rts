//! Lowering content scenarios into sim configuration.
//!
//! Content types describe what a scenario wants; the M0 sim supports a subset.
//! Anything the sim cannot honour yet is reported as an [`Issue`] rather than
//! silently approximated, because a silently different battle would make
//! benchmark and determinism results misleading.

use frontline_content::{HeightfieldDef, Issue, Scenario};
use frontline_sim::{SimConfig, TerrainKind, TickRate};

/// Peak amplitude (m) of the M0 sim's fixed noise terrain (12 m + 2 m octaves).
pub const SIM_NOISE_AMPLITUDE_M: u32 = 14;

/// Every problem with running `scenario` at world seed `seed`: content
/// validation first, then what the current sim cannot do. Empty means
/// [`sim_config`] will succeed.
pub fn check(scenario: &Scenario, seed: u64) -> Vec<Issue> {
    let mut issues = scenario.validate();
    issues.extend(unsupported(scenario, seed));
    issues
}

/// Valid content the M0 sim cannot run yet.
pub fn unsupported(scenario: &Scenario, seed: u64) -> Vec<Issue> {
    let mut issues = Vec::new();
    if let HeightfieldDef::Noise {
        seed: terrain_seed,
        amplitude_m,
    } = scenario.map.heightfield
    {
        if let Some(ts) = terrain_seed
            && ts != seed
        {
            issues.push(
                Issue::new(
                    "map.heightfield.seed",
                    format!(
                        "a terrain seed ({ts}) different from the world seed ({seed}) is not \
                         supported by the M0 sim; its terrain always uses the world seed"
                    ),
                )
                .with_hint("remove `seed` from the heightfield"),
            );
        }
        if amplitude_m != SIM_NOISE_AMPLITUDE_M {
            issues.push(
                Issue::new(
                    "map.heightfield.amplitude_m",
                    format!(
                        "the M0 sim generates noise with a fixed {SIM_NOISE_AMPLITUDE_M} m \
                         amplitude, got {amplitude_m}"
                    ),
                )
                .with_hint(format!("amplitude_m = {SIM_NOISE_AMPLITUDE_M}, or omit it")),
            );
        }
    }
    issues
}

/// Builds the sim config for `scenario` with world seed `seed`, or returns
/// every problem found by [`check`].
pub fn sim_config(scenario: &Scenario, seed: u64) -> Result<SimConfig, Vec<Issue>> {
    let issues = check(scenario, seed);
    if !issues.is_empty() {
        return Err(issues);
    }
    let tick_rate = if scenario.tick_hz == 20 {
        TickRate::Hz20
    } else {
        TickRate::Hz30
    };
    let terrain = match scenario.map.heightfield {
        HeightfieldDef::Flat => TerrainKind::Flat,
        HeightfieldDef::Noise { .. } => TerrainKind::Noise,
    };

    // A soldier fires, then waits `cooldown` ticks, so the period is
    // cooldown + 1 ticks. Validation guarantees a period of at least 1 tick
    // and at most 60 s (1,800 ticks), which fits the sim's u16 cooldown.
    let (pmin, pmax) = scenario.fire_period_ticks();
    let cooldown = |p: u32| p.saturating_sub(1).min(u16::MAX as u32) as u16;

    let mut config = SimConfig {
        seed,
        tick_rate,
        hash_interval: scenario.hash_interval,
        map_size_m: scenario.map.size_m as i32,
        terrain,
        soldiers: scenario.forces.soldiers,
        players: scenario.forces.players,
        projectile_cap: scenario.projectiles.pool_cap,
        ..SimConfig::default()
    };
    config.weapon.cooldown_min = cooldown(pmin);
    config.weapon.cooldown_max = cooldown(pmax);

    // Content validation mirrors the sim's rules, so this only fires if the
    // two drift apart.
    config
        .validate()
        .map_err(|e| vec![Issue::new("(sim)", e.to_string())])?;
    Ok(config)
}
