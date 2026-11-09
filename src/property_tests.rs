//! Generated invariants; failing inputs are shrunk and saved by proptest.
use super::*;
use proptest::prelude::*;

fn alice() -> User {
    User {
        name: "Alice".into(),
        role: "player".into(),
    }
}

proptest! {
    #[test]
    fn dice_totals_respect_mathematical_bounds(count in 1u32..=50, sides in 2u32..=1000, modifier in -1000i64..=1000) {
        let formula = format!(" {count}D{sides} {modifier:+} ");
        let result = roll(&formula).unwrap();
        prop_assert_eq!(result.formula, format!("{count}d{sides}{modifier:+}"));
        prop_assert!((i64::from(count) + modifier..=i64::from(count * sides) + modifier).contains(&result.total));
        let negative = roll(&format!("-{count}d{sides}{modifier:+}")).unwrap();
        prop_assert!((-i64::from(count * sides) + modifier..=-i64::from(count) + modifier).contains(&negative.total));
    }

    #[test]
    fn aggregate_dice_quota_cannot_be_bypassed(a in 51u32..=100, b in 50u32..=100, sides in 2u32..=100000) {
        let formula = format!("{a}d{sides}+{b}d{sides}");
        prop_assert!(roll(&formula).is_err());
    }

    #[test]
    fn arbitrary_dice_input_never_panics(formula in any::<String>()) {
        // Either a valid roll or a controlled API error; a panic fails the property.
        if let Err((status, _)) = roll(&formula) {
            prop_assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn numeric_bounds_and_fractional_hp_are_enforced(min in -10000i32..10000, span in 1i32..10000, fraction in 1u32..1000) {
        let max = min + span;
        for n in [min, min + span / 2, max] {
            prop_assert_eq!(num(&json!({"n":n}), "n", f64::from(min), f64::from(max)).unwrap(), f64::from(n));
        }
        for n in [min - 1, max + 1] {
            prop_assert!(num(&json!({"n":n}), "n", f64::from(min), f64::from(max)).is_err(), "out-of-range value accepted");
        }
        prop_assert!(max_hp(&json!({"max_hp":1. + f64::from(fraction) / 1000.})).is_err(), "fractional HP accepted");
    }

    #[test]
    fn fog_membership_is_translation_invariant(x in 0i32..1000, y in 0i32..1000, width in 1i32..1000, height in 1i32..1000, dx in -1000i32..1000, dy in -1000i32..1000) {
        let rect = json!({"x":x,"y":y,"w":width,"h":height});
        let translated = json!({"x":x+dx,"y":y+dy,"w":width,"h":height});
        for (px, py, expected) in [(x,y,true), (x+width,y+height,true), (x-1,y,false), (x,y-1,false), (x+width+1,y,false), (x,y+height+1,false)] {
            prop_assert_eq!(covered(&json!({"x":px,"y":py}), &[rect.clone()]), expected);
            prop_assert_eq!(covered(&json!({"x":px+dx,"y":py+dy}), &[translated.clone()]), expected);
        }
    }

    #[test]
    fn walls_block_crossings_in_both_directions(x in 100i32..900, y in 100i32..900, distance in 1i32..99, half_height in 1i32..99, transpose in any::<bool>()) {
        let point = |a: i32, b: i32| if transpose { (b,a) } else { (a,b) };
        let a = point(x-distance,y);
        let b = point(x+distance,y);
        let c = point(x,y-half_height);
        let d = point(x,y+half_height);
        let scene = json!({"collision":true,"walls":[{"x1":c.0,"y1":c.1,"x2":d.0,"y2":d.1}]});
        for (start, end) in [(a,b),(b,a)] {
            let token = json!({"x":start.0,"y":start.1});
            prop_assert!(scene_tools::check_move(&scene, &token, (f64::from(end.0),f64::from(end.1)), &alice()).is_err());
        }
        // The same path beyond the wall's endpoint must remain traversable.
        let start = point(x-distance,y+half_height+1);
        let end = point(x+distance,y+half_height+1);
        prop_assert!(scene_tools::check_move(&scene, &json!({"x":start.0,"y":start.1}), (f64::from(end.0),f64::from(end.1)), &alice()).is_ok(), "path outside wall endpoint blocked");
    }

    #[test]
    fn projection_preserves_source_and_hides_generated_private_data(hidden in any::<bool>(), fogged in any::<bool>(), secret in "[a-zA-Z0-9]{1,64}", turn in 0u32..100) {
        let mut world = demo();
        world["actors"][0]["owner"] = json!("Bob");
        world["actors"][0]["notes"] = json!(secret.clone());
        world["builder_drafts"] = json!([{"id":"draft","owner":"Bob","data":{"secret":secret}}]);
        world["scenes"][0]["tokens"][1]["hidden"] = json!(hidden);
        if fogged { world["scenes"][0]["fog"] = json!([{"x":700,"y":300,"w":100,"h":100}]); }
        world["combat"] = json!({"order":[{"id":"wolf"},{"id":"hero"}],"turn":turn,"round":1});
        let original = world.clone();
        let projected = snapshot(&world, &alice(), 42);
        prop_assert_eq!(&world, &original);
        prop_assert_eq!(&projected["world"]["actors"][0]["notes"], &json!(""));
        prop_assert_eq!(&projected["world"]["builder_drafts"], &json!([]));
        let tokens = projected["world"]["scenes"][0]["tokens"].as_array().unwrap();
        let combat = projected["world"]["combat"]["order"].as_array().unwrap();
        prop_assert_eq!(tokens.iter().any(|t| t["id"] == "wolf"), !(hidden || fogged));
        prop_assert_eq!(combat.iter().any(|t| t["id"] == "wolf"), !(hidden || fogged));
        prop_assert!(projected["world"]["combat"]["turn"].as_u64().unwrap() < combat.len() as u64);
        let round_trip: Value = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
        prop_assert_eq!(round_trip, original);
    }

    #[test]
    fn denied_actions_never_change_world(x in 0u32..1600, y in 0u32..1000, hp in -10000i32..10000) {
        let original = demo();
        for command in [json!({"kind":"fog_clear"}), json!({"kind":"move_token","id":"wolf","x":x,"y":y}), json!({"kind":"update_token","id":"wolf","hp":hp})] {
            let mut world = original.clone();
            prop_assert_eq!(apply(&mut world, &alice(), &command).unwrap_err().0, StatusCode::FORBIDDEN);
            prop_assert_eq!(world, original.clone());
        }
    }

    #[test]
    fn active_effects_expire_but_paused_and_nonparticipants_do_not(rounds in 1u32..=1000, elapsed in 1u32..=1000, paused in any::<bool>()) {
        let effect = json!({"id":"effect","rounds":rounds,"disabled":paused});
        let mut world = json!({"actors":[{"id":"a","hp":17,"effects":[effect.clone()]},{"id":"b","hp":12,"effects":[effect.clone()]}],"combat":{"order":[{"id":"a"}]}});
        let outside = world["actors"][1].clone();
        for _ in 0..elapsed { effects::advance_round(&mut world); }
        let remaining = world["actors"][0]["effects"].as_array().unwrap();
        if paused { prop_assert_eq!(remaining, &vec![effect]); }
        else if elapsed >= rounds { prop_assert!(remaining.is_empty()); }
        else { prop_assert_eq!(remaining[0]["rounds"].as_u64(), Some(u64::from(rounds-elapsed))); }
        prop_assert_eq!(&world["actors"][1], &outside);
        prop_assert_eq!(&world["actors"][0]["hp"], &json!(17));
    }
}
