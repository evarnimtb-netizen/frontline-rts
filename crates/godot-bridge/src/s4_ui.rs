//! S4 presenter words (plain Rust, unit tested without Godot): member slot
//! names as role words, a card's tooltip gloss, and the tags and
//! capabilities the HUD reads (`godot/docs/match_state.md` "S4").

use frontline_content::MatchSetup;
use frontline_content::setup::{Capability, SquadTypeSetup};

/// A `[[members]]` slot name as a role word: `mg_gunner` -> "MG gunner",
/// `asst_ar` -> "Asst AR", `rifleman_pzf` -> "Rifleman (Pzf)".
pub fn slot_word(slot: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for (k, w) in slot.split('_').filter(|w| !w.is_empty()).enumerate() {
        let up = match w {
            "mg" | "ar" | "smg" | "at" | "hq" => w.to_uppercase(),
            "pzf" => "(Pzf)".to_owned(),
            "asst" => "Asst".to_owned(),
            _ if k == 0 => {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
            _ => w.to_owned(),
        };
        words.push(up);
    }
    words.join(" ")
}

/// The English gloss of a card's `type` word ("rifle squad").
fn type_gloss(word: &str) -> &'static str {
    match word {
        "rifle" => "rifle squad",
        "mg" => "machine-gun squad",
        "engineer" => "engineer squad",
        "mortar" => "mortar team",
        "at" => "anti-tank team",
        "vehicle" => "tank",
        "crew" => "tank crew",
        _ => "squad",
    }
}

/// The call-in tile tooltip line (S4-13 (A), period name with an English
/// gloss): "Grenadiergruppe — rifle squad, 10 men: 1 MP 40, 1 MG 42, 1 P 38,
/// 7 Kar98k; 2 Panzerfaust 60".
pub fn card_gloss(setup: &MatchSetup, t: u16) -> String {
    let Some(ty) = setup.squad_types.get(t as usize) else {
        return String::new();
    };
    let head = format!("{} — {}", ty.name, type_gloss(setup.type_word(t)));
    if ty.vehicle.is_some() || ty.members.is_empty() {
        return head;
    }
    // Weapons in order of first appearance, with their men.
    let mut weapons: Vec<(u16, u32)> = Vec::new();
    for m in &ty.members {
        match weapons.iter_mut().find(|(w, _)| *w == m.weapon) {
            Some(e) => e.1 += m.count as u32,
            None => weapons.push((m.weapon, m.count as u32)),
        }
    }
    let name = |w: u16| {
        setup
            .weapons
            .get(w as usize)
            .map_or("?", |w| w.short_name.as_str())
            .to_owned()
    };
    let arms: Vec<String> = weapons
        .iter()
        .map(|&(w, n)| format!("{n} {}", name(w)))
        .collect();
    let mut out = format!("{head}, {} men: {}", ty.men, arms.join(", "));
    // Single-shot AT items (Panzerfausts) by count.
    let k = frontline_content::ItemKind::SingleShotAt;
    let n = ty.squad_items(k);
    if n > 0
        && let Some((_, it)) = ty.kit(&setup.items, k)
    {
        out += &format!("; {n} {}", it.name);
    }
    out
}

/// The card's tags as words.
pub fn tag_words(setup: &MatchSetup, ty: &SquadTypeSetup) -> Vec<String> {
    ty.tags
        .iter()
        .filter_map(|&t| setup.tags.get(t as usize).cloned())
        .collect()
}

/// The card's capabilities as their TOML names.
pub fn capability_words(ty: &SquadTypeSetup) -> Vec<&'static str> {
    Capability::ALL
        .iter()
        .filter(|c| ty.has(**c))
        .map(|c| c.name())
        .collect()
}

/// Whether the card carries the reserved `understrength` tag (the HUD's
/// half chevron).
pub fn understrength(setup: &MatchSetup, ty: &SquadTypeSetup) -> bool {
    ty.has_tag(setup.tag("understrength"))
}

/// The role word of formation slot `k` (spawn order) of `ty`.
pub fn spawn_slot_word(ty: &SquadTypeSetup, k: usize) -> String {
    ty.members
        .get(ty.slot_of(k) as usize)
        .map_or_else(String::new, |m| slot_word(&m.slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_become_role_words() {
        assert_eq!(slot_word("mg_gunner"), "MG gunner");
        assert_eq!(slot_word("asst_ar"), "Asst AR");
        assert_eq!(slot_word("rifleman_pzf"), "Rifleman (Pzf)");
        assert_eq!(slot_word("leader"), "Leader");
        assert_eq!(slot_word("asst_squad_leader"), "Asst squad leader");
    }

    #[test]
    fn a_grenadiergruppe_card_reads_as_a_rifle_squad_with_its_weapons() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let s = defs
            .resolve(&frontline_content::MatchRequest::skirmish(
                1,
                "ww2:germany",
                "ww2:usa",
                frontline_content::Difficulty::Normal,
            ))
            .unwrap();
        let t = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
        assert_eq!(
            card_gloss(&s, t),
            "Grenadiergruppe — rifle squad, 10 men: 1 MP 40, 1 MG 42, 1 P 38, 7 Kar98k; \
             2 Panzerfaust 60"
        );
        let ty = &s.squad_types[t as usize];
        assert_eq!(tag_words(&s, ty), ["line"]);
        assert_eq!(capability_words(ty), ["dig_basic", "recrew", "at_hunt"]);
        assert!(!understrength(&s, ty));
        let t7 = s.squad_type_index("ww2:ger_grenadiergruppe_7").unwrap();
        assert!(understrength(&s, &s.squad_types[t7 as usize]));
        assert_eq!(spawn_slot_word(ty, 1), "MG gunner");
        assert_eq!(spawn_slot_word(ty, 9), "Rifleman");
    }
}
