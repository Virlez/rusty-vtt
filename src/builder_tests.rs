use super::*;

fn fixture(class_id: &str) -> (Value, Value) {
    let mut world = demo();
    world["modules"] = json!({ID:{"enabled":true}});
    let user = User {
        name: "MJ".into(),
        role: "gm".into(),
    };
    apply(&mut world, &user, &json!({"kind":"builder_draft_create"})).unwrap();
    let mut draft = world["builder_drafts"][0].clone();
    let class = lookup("classes", class_id).unwrap();
    draft["data"]["name"] = json!("Generated fixture");
    draft["data"]["class"] = json!(class_id);
    draft["data"]["species"] = json!("nain");
    draft["data"]["extra_skills"] = json!([]);
    let background = lookup("backgrounds", "soldat").unwrap();
    draft["data"]["skills"] = json!(
        list(&class["skills"])
            .into_iter()
            .filter(|s| !list(&background["skills"]).contains(s))
            .take(class["skill_count"].as_u64().unwrap() as usize)
            .collect::<Vec<_>>()
    );
    if class["casting"].is_string() {
        let spells: Vec<&Value> = catalog()["spells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| list(&s["classes"]).contains(&class["name"].as_str().unwrap().to_string()))
            .collect();
        let p = progress(class, 1);
        let cantrips: Vec<Value> = spells
            .iter()
            .filter(|s| s["level"] == 0)
            .take(p["cantrips"].as_u64().unwrap_or(0) as usize)
            .map(|s| s["id"].clone())
            .collect();
        let book: Vec<Value> = spells
            .iter()
            .filter(|s| s["level"] == 1)
            .take(if class_id == "magicien" {
                6
            } else {
                p["prepared"].as_u64().unwrap_or(0) as usize
            })
            .map(|s| s["id"].clone())
            .collect();
        let prepared: Vec<Value> = book
            .iter()
            .take(p["prepared"].as_u64().unwrap_or(0) as usize)
            .cloned()
            .collect();
        draft["data"]["spell_choices"][class_id] = json!({"cantrips":cantrips,"prepared":prepared,"book":if class_id=="magicien" {book} else {vec![]}});
    }
    (world, draft)
}

#[test]
fn every_class_can_create_a_valid_character() {
    for class in catalog()["classes"].as_array().unwrap() {
        let id = class["id"].as_str().unwrap();
        let (world, draft) = fixture(id);
        let actor = assemble(&world, &draft).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        builder_validation::actor(&actor).unwrap();
        assert_eq!(actor["dnd5e"]["level"], 1);
        assert_eq!(actor["dnd5e"]["classes"][0]["id"], id);
        let (hp, armor) = match id {
            "barbare" => (15, 14),
            "barde" | "occultiste" | "roublard" => (11, 13),
            "clerc" => (11, 17),
            "druide" => (11, 15),
            "ensorceleur" | "magicien" => (9, 12),
            "guerrier" => (13, 16),
            "moine" => (11, 12),
            "paladin" => (13, 18),
            "rodeur" => (13, 14),
            other => panic!("Add explicit expectations for {other}"),
        };
        assert_eq!(actor["max_hp"], hp, "{id}");
        assert_eq!(actor["armor"], armor, "{id}");
    }
}

#[test]
fn wizard_rejects_duplicate_unknown_and_unprepared_book_entries() {
    let (world, valid) = fixture("magicien");
    assert!(assemble(&world, &valid).is_ok());
    for kind in ["duplicate", "unknown", "outside_book", "missing_book"] {
        let mut draft = valid.clone();
        let spells = &mut draft["data"]["spell_choices"]["magicien"];
        match kind {
            "duplicate" => spells["prepared"][1] = spells["prepared"][0].clone(),
            "unknown" => spells["cantrips"][0] = json!("does-not-exist"),
            "outside_book" => spells["book"][0] = json!("does-not-exist"),
            _ => spells["book"] = json!([]),
        }
        assert!(assemble(&world, &draft).is_err(), "{kind}");
    }
}

#[test]
fn multiclass_requires_permission_and_both_ability_prerequisites() {
    let (mut world, mut draft) = fixture("guerrier");
    let actor = assemble(&world, &draft).unwrap();
    world["actors"].as_array_mut().unwrap().push(actor.clone());
    draft["mode"] = json!("levelup");
    draft["actor_id"] = actor["id"].clone();
    draft["data"]["class"] = json!("roublard");
    assert!(assemble(&world, &draft).is_err());
    world["builder_settings"] = json!({"allow_multiclass":true});
    let combined = assemble(&world, &draft).unwrap();
    assert_eq!(combined["dnd5e"]["level"], 2);
    assert_eq!(combined["dnd5e"]["classes"].as_array().unwrap().len(), 2);
    // Change the historical score too: fail the prerequisite rather than history preservation.
    draft["data"]["scores"]["dex"] = json!(8);
    world["actors"][1]["dnd5e"]["build"]["scores"]["dex"] = json!(8);
    assert!(
        assemble(&world, &draft)
            .unwrap_err()
            .iter()
            .any(|e| e.contains("13"))
    );
}

#[test]
fn equipment_budget_and_progression_history_are_enforced() {
    let (mut world, draft) = fixture("guerrier");
    for cart in [
        json!({"unknown":1}),
        json!({"bouclier":-1}),
        json!({"bouclier":1000}),
    ] {
        let mut bad = draft.clone();
        bad["data"]["class_equipment"] = json!("gold");
        bad["data"]["cart"] = cart;
        assert!(assemble(&world, &bad).is_err());
    }
    let actor = assemble(&world, &draft).unwrap();
    world["actors"].as_array_mut().unwrap().push(actor.clone());
    let mut level = draft;
    level["mode"] = json!("levelup");
    level["actor_id"] = actor["id"].clone();
    let next = assemble(&world, &level).unwrap();
    assert_eq!(next["dnd5e"]["level"], 2);
    assert!(next["max_hp"].as_i64().unwrap() > actor["max_hp"].as_i64().unwrap());
    level["data"]["species"] = json!("humain");
    assert!(assemble(&world, &level).is_err());
}

#[test]
fn malformed_draft_numbers_return_errors_without_overflow() {
    let (world, draft) = fixture("magicien");
    for n in [i64::MAX, i64::MIN] {
        let mut bad = draft.clone();
        bad["data"]["scores"]["str"] = json!(n);
        bad["data"]["boosts"]["str"] = json!(n);
        assert!(assemble(&world, &bad).is_err());
    }
}

#[test]
fn fourth_fighter_level_requires_two_asi_points_and_preserves_inventory() {
    let (mut world, mut draft) = fixture("guerrier");
    let mut actor = assemble(&world, &draft).unwrap();
    world["actors"].as_array_mut().unwrap().push(actor.clone());
    draft["mode"] = json!("levelup");
    draft["actor_id"] = actor["id"].clone();
    for level in [2, 3] {
        actor = assemble(&world, &draft).unwrap();
        assert_eq!(actor["dnd5e"]["level"], level);
        world["actors"][1] = actor.clone();
    }
    assert!(assemble(&world, &draft).is_err());
    draft["data"]["asi"] = json!({"str":2});
    let improved = assemble(&world, &draft).unwrap();
    assert_eq!(improved["dnd5e"]["level"], 4);
    assert_eq!(improved["dnd5e"]["abilities"]["str"], 19);
    assert_eq!(improved["items"], actor["items"]);
    assert_eq!(
        improved["dnd5e"]["currency_cp"],
        actor["dnd5e"]["currency_cp"]
    );
    draft["data"]["asi"]["str"] = json!(3);
    assert!(assemble(&world, &draft).is_err());
}
