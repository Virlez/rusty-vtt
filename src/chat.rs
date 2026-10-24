//! Native message visibility. Secrets are removed before snapshots leave the server.
use crate::{User, bad, collection, id, roll, text};
use axum::http::StatusCode;
use serde_json::{Value, json};

pub fn apply(w: &mut Value, u: &User, v: &Value) -> Result<bool, (StatusCode, String)> {
    let kind = v["kind"].as_str().unwrap_or("");
    if !matches!(kind, "chat" | "roll") {
        return Ok(false);
    }
    let mode = v.get("mode").and_then(Value::as_str).unwrap_or("public");
    if !matches!(mode, "public" | "gm" | "blind" | "self" | "whisper") {
        return Err(bad("Mode de conversation invalide"));
    }
    if kind != "roll" && mode == "blind" {
        return Err(bad("Le mode aveugle est réservé aux jets"));
    }
    let mut recipients = Vec::<String>::new();
    if mode == "whisper" {
        let names = v["recipients"]
            .as_array()
            .ok_or(bad("Destinataires requis"))?;
        if names.is_empty() || names.len() > 20 {
            return Err(bad("Indiquez 1 à 20 destinataires"));
        }
        for name in names {
            let name = name.as_str().unwrap_or("").trim();
            if name.is_empty() || name.len() > 60 {
                return Err(bad("Destinataire invalide"));
            }
            if !recipients.iter().any(|n| n == name) {
                recipients.push(name.into());
            }
        }
    }
    let (message, result) = if kind == "roll" {
        let r = roll(&text(v, "formula", 80)?)?;
        (
            format!("{} = {}", r.formula, r.total),
            serde_json::to_value(r).unwrap(),
        )
    } else {
        (text(v, "text", 4000)?, Value::Null)
    };
    let messages = collection(w, "messages")?;
    messages.push(
        json!({"id":id(),"author":u.name,"text":message,"roll":result,
        "mode":mode,"recipients":recipients}),
    );
    if messages.len() > 500 {
        messages.remove(0);
    }
    Ok(true)
}

pub fn project(w: &mut Value, u: &User) {
    if let Some(messages) = w["messages"].as_array_mut() {
        messages.retain(|m| {
            let author = m["author"].as_str() == Some(u.name.as_str());
            match m["mode"].as_str().unwrap_or("public") {
                "public" => true,
                "gm" => author || u.role == "gm",
                "blind" => u.role == "gm",
                "self" => author,
                "whisper" => {
                    author
                        || m["recipients"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|n| n.as_str() == Some(u.name.as_str())))
                }
                _ => false,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_evicts_oldest_and_normalizes_recipients() {
        let mut w = json!({"messages": (0..500).map(|i| json!({"text":i})).collect::<Vec<_>>()});
        apply(
            &mut w,
            &user("Alice", "player"),
            &json!({
                "kind":"chat", "text":"new", "mode":"whisper", "recipients":[" Bob ","Bob"]
            }),
        )
        .unwrap();
        let messages = w["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 500);
        assert_eq!(messages[0]["text"], 1);
        assert_eq!(messages[499]["recipients"], json!(["Bob"]));
    }

    #[test]
    fn invalid_recipients_leave_history_unchanged() {
        let original = json!({"messages":[]});
        for recipients in [
            json!(null),
            json!([42]),
            json!([""]),
            json!([" "]),
            json!(vec!["Bob"; 21]),
        ] {
            let mut world = original.clone();
            assert!(
                apply(
                    &mut world,
                    &user("Alice", "player"),
                    &json!({
                        "kind":"chat", "text":"secret", "mode":"whisper", "recipients":recipients
                    })
                )
                .is_err()
            );
            assert_eq!(world, original);
        }
    }
    fn user(name: &str, role: &str) -> User {
        User {
            name: name.into(),
            role: role.into(),
        }
    }
    #[test]
    fn visibility_matrix_and_legacy_public() {
        let original = json!({"messages":[
            {"author":"Alice","text":"legacy"},
            {"author":"Alice","mode":"public","text":"public"},
            {"author":"Alice","mode":"gm","text":"gm secret"},
            {"author":"Alice","mode":"blind","roll":{"total":98765}},
            {"author":"Alice","mode":"self","text":"self secret"},
            {"author":"Alice","mode":"whisper","recipients":["Bob"],"text":"whisper secret"}
        ]});
        for (name, role, expected) in [
            ("Alice", "player", 5),
            ("Bob", "player", 3),
            ("Eve", "player", 2),
            ("MJ", "gm", 4),
        ] {
            let mut view = original.clone();
            project(&mut view, &user(name, role));
            assert_eq!(view["messages"].as_array().unwrap().len(), expected);
            if role != "gm" {
                assert!(!view.to_string().contains("98765"));
            }
        }
        assert_eq!(original["messages"].as_array().unwrap().len(), 6);
    }
    #[test]
    fn blind_roll_is_evaluated_once_and_never_sent_to_player() {
        let mut w = json!({"messages":[]});
        let alice = user("Alice", "player");
        apply(
            &mut w,
            &alice,
            &json!({"kind":"roll","formula":"1d6+10","mode":"blind"}),
        )
        .unwrap();
        let total = w["messages"][0]["roll"]["total"].as_i64().unwrap();
        assert!((11..=16).contains(&total));
        let mut player_view = w.clone();
        project(&mut player_view, &alice);
        assert_eq!(player_view["messages"], json!([]));
        let mut gm_view = w.clone();
        project(&mut gm_view, &user("MJ", "gm"));
        assert_eq!(gm_view, w);
    }
    #[test]
    fn validates_before_mutation_and_cannot_spoof_author() {
        let mut w = json!({"messages":[]});
        let alice = user("Alice", "player");
        for invalid in [
            json!({"kind":"roll","formula":"1d6","mode":"oops"}),
            json!({"kind":"chat","text":"x","mode":"blind"}),
            json!({"kind":"chat","text":"x","mode":"whisper","recipients":[]}),
            json!({"kind":"roll","formula":"evil()"}),
        ] {
            assert!(apply(&mut w, &alice, &invalid).is_err());
        }
        assert_eq!(w["messages"], json!([]));
        assert!(apply(&mut w, &alice, &json!({"kind":"chat","text":"secret","mode":"whisper","recipients":["Bob"],"author":"Bob"})).unwrap());
        assert_eq!(w["messages"][0]["author"], "Alice");
        let mut eve_view = w.clone();
        project(&mut eve_view, &user("Eve", "player"));
        assert_eq!(eve_view["messages"], json!([]));
        assert!(!apply(&mut w, &alice, &json!({"kind":"move_token"})).unwrap());
    }
}
