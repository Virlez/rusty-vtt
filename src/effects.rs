//! Native descriptive conditions, with optional full-combat-round durations.
use super::*;

pub fn validate(w: &Value) -> Result<(), (StatusCode, String)> {
    for actor in w["actors"].as_array().ok_or(bad("Personnages invalides"))? {
        if let Some(value) = actor.get("effects") {
            if value.is_null() { continue; }
            let list = value.as_array().ok_or(bad("Liste d’états invalide"))?;
            if list.len() > 50 {
                return Err(bad("Maximum de 50 états par personnage"));
            }
            for effect in list {
                text(effect, "id", 100)?;
                text(effect, "name", 80)?;
                if !effect["rounds"].is_null()
                    && !effect["rounds"]
                        .as_u64()
                        .is_some_and(|n| (1..=1000).contains(&n))
                {
                    return Err(bad("Durée d’état invalide"));
                }
                if !effect["disabled"].is_boolean()
                    || effect["description"]
                        .as_str()
                        .is_none_or(|s| s.chars().count() > 2000)
                {
                    return Err(bad("État invalide"));
                }
            }
        }
    }
    Ok(())
}

pub fn apply(w: &mut Value, u: &User, v: &Value) -> Result<bool, (StatusCode, String)> {
    let kind = v["kind"].as_str().unwrap_or("");
    if !kind.starts_with("effect_") {
        return Ok(false);
    }
    if !["effect_add", "effect_remove", "effect_toggle"].contains(&kind) {
        return Err(bad("Action d’état inconnue"));
    }
    // Validate before mutating, including for direct callers outside the HTTP transaction.
    let new_effect = if kind == "effect_add" {
        let name = text(v, "name", 80)?;
        let rounds = match &v["rounds"] {
            Value::Null => Value::Null,
            x => json!(
                x.as_u64()
                    .filter(|n| (1..=1000).contains(n))
                    .ok_or(bad("La durée doit être comprise entre 1 et 1000 rounds"))?
            ),
        };
        let description = v["description"].as_str().unwrap_or("");
        if description.chars().count() > 2000 {
            return Err(bad("Description trop longue"));
        }
        Some(
            json!({"id":id(),"name":name,"description":description,"rounds":rounds,"disabled":false}),
        )
    } else {
        None
    };
    let actor = find(collection(w, "actors")?, &v["id"])?;
    if !owned(u, actor) {
        return Err(forbidden());
    }
    if kind == "effect_add" {
        if actor["effects"].as_array().map_or(0, Vec::len) >= 50 {
            return Err(bad("Maximum de 50 états par personnage"));
        }
        if !actor["effects"].is_array() {
            actor["effects"] = json!([]);
        }
        actor["effects"]
            .as_array_mut()
            .unwrap()
            .push(new_effect.unwrap());
    } else {
        let effects = actor["effects"]
            .as_array_mut()
            .ok_or(bad("État introuvable"))?;
        let index = effects
            .iter()
            .position(|e| e["id"] == v["effect_id"])
            .ok_or(bad("État introuvable"))?;
        if kind == "effect_remove" {
            effects.remove(index);
        } else {
            effects[index]["disabled"] =
                json!(!effects[index]["disabled"].as_bool().unwrap_or(false));
        }
    }
    Ok(true)
}

/// Call exactly once when combat_next wraps to the next round. Only participants age.
/// Disabled conditions are paused. Expired conditions are removed; HP is never changed.
pub fn advance_round(w: &mut Value) {
    let participants: Vec<Value> = w["combat"]["order"]
        .as_array()
        .map(|order| {
            order
                .iter()
                .map(|e| {
                    if e.is_string() {
                        e.clone()
                    } else {
                        e["id"].clone()
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(actors) = w["actors"].as_array_mut() {
        for actor in actors
            .iter_mut()
            .filter(|a| participants.contains(&a["id"]))
        {
            if let Some(effects) = actor.get_mut("effects").and_then(Value::as_array_mut) {
                for effect in effects.iter_mut() {
                    if effect["disabled"] == true {
                        continue;
                    }
                    if let Some(n) = effect["rounds"].as_u64() {
                        effect["rounds"] = json!(n.saturating_sub(1));
                    }
                }
                effects.retain(|e| e["disabled"] == true || e["rounds"].as_u64() != Some(0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn player() -> User {
        User {
            name: "Alice".into(),
            role: "player".into(),
        }
    }
    fn world() -> Value {
        json!({"actors":[{"id":"a","owner":"Alice"},{"id":"b","owner":"Bob"}],"combat":{"order":[{"id":"a"}]}})
    }
    #[test]
    fn ownership_and_invalid_duration() {
        let mut w = world();
        let before = w.clone();
        assert!(
            apply(
                &mut w,
                &player(),
                &json!({"kind":"effect_add","id":"b","name":"Blessé"})
            )
            .is_err()
        );
        assert!(
            apply(
                &mut w,
                &player(),
                &json!({"kind":"effect_add","id":"a","name":"Blessé","rounds":0})
            )
            .is_err()
        );
        assert_eq!(w, before);
    }
    #[test]
    fn duration_pause_expiry_and_participants() {
        let mut w = world();
        for rounds in [json!(1), json!(2), Value::Null] {
            apply(
                &mut w,
                &player(),
                &json!({"kind":"effect_add","id":"a","name":"État","rounds":rounds}),
            )
            .unwrap();
        }
        w["actors"][0]["effects"][1]["disabled"] = json!(true);
        w["actors"][1]["effects"] = json!([{"id":"x","rounds":2}]);
        advance_round(&mut w);
        assert_eq!(w["actors"][0]["effects"].as_array().unwrap().len(), 2);
        assert_eq!(w["actors"][0]["effects"][0]["rounds"], 2);
        assert!(w["actors"][0]["effects"][1]["rounds"].is_null());
        assert_eq!(w["actors"][1]["effects"][0]["rounds"], 2);
        let effect_id = w["actors"][0]["effects"][0]["id"].clone();
        apply(
            &mut w,
            &player(),
            &json!({"kind":"effect_toggle","id":"a","effect_id":effect_id}),
        )
        .unwrap();
        advance_round(&mut w);
        advance_round(&mut w);
        assert_eq!(w["actors"][0]["effects"].as_array().unwrap().len(), 1);
    }
}
