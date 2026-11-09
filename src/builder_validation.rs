//! Validate imported 5e structures before they reach indexing and dice operations.
use super::*;

fn object_fields(value: &Value, fields: &[&str]) -> Result<(), (StatusCode, String)> {
    for key in fields {
        if value.get(key).is_some_and(|v| !v.is_object()) {
            return Err(bad("Structure 5e imbriquée invalide"));
        }
    }
    Ok(())
}

pub fn actor(actor: &Value) -> Result<(), (StatusCode, String)> {
    if actor["system_id"] == "dnd5e" {
        num(actor, "hp", -10000., 10000.)?;
    }
    let Some(data) = actor.get("dnd5e") else {
        return if actor["system_id"] == "dnd5e" {
            Err(bad("Données 5e manquantes"))
        } else {
            Ok(())
        };
    };
    if !data.is_object() {
        return Err(bad("Données 5e invalides"));
    }
    object_fields(data, &["build", "abilities", "improvements", "spent_slots"])?;
    if let Some(build) = data.get("build") {
        object_fields(
            build,
            &[
                "scores",
                "boosts",
                "asi",
                "cart",
                "spell_choices",
                "origin_magic",
            ],
        )?;
        for (key, min, max) in [("scores", 3, 18), ("boosts", 0, 2), ("asi", 0, 2)] {
            if let Some(scores) = build[key].as_object()
                && scores
                    .values()
                    .any(|v| !v.as_i64().is_some_and(|n| (min..=max).contains(&n)))
            {
                return Err(bad("Caractéristiques d’origine invalides"));
            }
        }
    }
    let mut total = 0;
    if let Some(classes) = data.get("classes") {
        let classes = classes
            .as_array()
            .filter(|c| !c.is_empty())
            .ok_or(bad("Classes 5e invalides"))?;
        let mut seen = std::collections::HashSet::new();
        for class in classes {
            let id = class["id"].as_str().ok_or(bad("Classe 5e invalide"))?;
            let known = builder::catalog()["classes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == id);
            let level = class["level"]
                .as_u64()
                .filter(|n| (1..=20).contains(n))
                .ok_or(bad("Niveau de classe invalide"))?;
            if known.is_none() || !seen.insert(id) {
                return Err(bad("Classe inconnue ou dupliquée"));
            }
            total += level;
        }
        if total > 20 || data["level"].as_u64() != Some(total) {
            return Err(bad("Niveau total 5e incohérent"));
        }
    }
    if let Some(level) = data.get("level")
        && !level.as_u64().is_some_and(|n| (1..=20).contains(&n))
    {
        return Err(bad("Niveau 5e invalide"));
    }
    if actor["system_id"] == "dnd5e" && (!data["build"].is_object() || !data["classes"].is_array())
    {
        return Err(bad("Fiche 5e incomplète : build et classes requis"));
    }
    for key in ["abilities", "improvements"] {
        if let Some(values) = data[key].as_object()
            && values
                .values()
                .any(|v| !v.as_i64().is_some_and(|n| (0..=30).contains(&n)))
        {
            return Err(bad("Caractéristiques 5e invalides"));
        }
    }
    if let Some(dice) = data.get("hp_dice") {
        let dice = dice.as_array().ok_or(bad("Dés de vie invalides"))?;
        if dice.len() > 20
            || dice.iter().any(|d| {
                !d["die"]
                    .as_u64()
                    .is_some_and(|n| [6, 8, 10, 12].contains(&n))
                    || !d["value"]
                        .as_u64()
                        .is_some_and(|n| n >= 1 && n <= d["die"].as_u64().unwrap_or(0))
            })
        {
            return Err(bad("Dés de vie invalides"));
        }
    }
    if let Some(spent) = data.get("hit_dice_spent")
        && !spent
            .as_u64()
            .is_some_and(|n| n <= data["level"].as_u64().unwrap_or(0))
    {
        return Err(bad("Dés de vie dépensés invalides"));
    }
    Ok(())
}

pub fn drafts(world: &Value) -> Result<(), (StatusCode, String)> {
    if let Some(settings) = world.get("builder_settings") {
        if !settings.is_object() {
            return Err(bad("Configuration 5e invalide"));
        }
        for key in [
            "allow_creation",
            "allow_manual",
            "allow_multiclass",
            "require_level_grant",
        ] {
            if settings.get(key).is_some_and(|v| !v.is_boolean()) {
                return Err(bad("Paramètre 5e booléen requis"));
            }
        }
        if settings
            .get("bonus_gold")
            .is_some_and(|v| !v.as_u64().is_some_and(|n| n <= 10000))
        {
            return Err(bad("Or bonus 5e invalide"));
        }
    }
    if let Some(drafts) = world["builder_drafts"].as_array() {
        for draft in drafts {
            if draft
                .get("version")
                .is_some_and(|v| !v.as_u64().is_some_and(|n| n < u32::MAX as u64))
            {
                return Err(bad("Version de brouillon invalide"));
            }
            if let Some(base) = draft.get("base_actor") {
                actor(base)?;
            }
            if draft
                .get("hp_roll")
                .is_some_and(|v| !v.as_u64().is_some_and(|n| (1..=12).contains(&n)))
            {
                return Err(bad("Tirage de PV invalide"));
            }
        }
    }
    Ok(())
}
