//! Independent scene geometry: authoritative movement, doors and limited token sight.
use super::*;
type P = (f64, f64);
fn point(v: &Value, x: &str, y: &str) -> P {
    (v[x].as_f64().unwrap_or(0.), v[y].as_f64().unwrap_or(0.))
}
fn cross(a: P, b: P) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
// Intersection along AB, including wall endpoints. Collinear overlap also blocks.
fn hit(a: P, b: P, c: P, d: P) -> Option<f64> {
    let r = sub(b, a);
    let s = sub(d, c);
    let q = sub(c, a);
    let denominator = cross(r, s);
    if denominator.abs() < 1e-9 {
        if cross(q, r).abs() > 1e-7 {
            return None;
        }
        let length = r.0 * r.0 + r.1 * r.1;
        if length < 1e-12 {
            return None;
        }
        let t1 = (q.0 * r.0 + q.1 * r.1) / length;
        let z = sub(d, a);
        let t2 = (z.0 * r.0 + z.1 * r.1) / length;
        let lo = t1.min(t2).max(1e-7);
        let hi = t1.max(t2).min(1.);
        return if lo <= hi { Some(lo) } else { None };
    }
    let t = cross(q, s) / denominator;
    let u = cross(q, r) / denominator;
    if t > 1e-7 && t <= 1. + 1e-9 && (-1e-9..=1. + 1e-9).contains(&u) {
        Some(t.min(1.))
    } else {
        None
    }
}
fn blocks(w: &Value, kind: &str) -> bool {
    !(w["door"] == true && w["state"] == "open") && w[kind] != false
}
fn blocked(s: &Value, a: P, b: P, kind: &str) -> bool {
    s["walls"].as_array().is_some_and(|ws| {
        ws.iter().any(|w| {
            blocks(w, kind) && hit(a, b, point(w, "x1", "y1"), point(w, "x2", "y2")).is_some()
        })
    })
}
pub fn check_move(s: &Value, t: &Value, p: P, u: &User) -> Result<(), (StatusCode, String)> {
    if u.role != "gm" && s["collision"] == true && blocked(s, point(t, "x", "y"), p, "blocks_move")
    {
        Err(bad("Un mur ou une porte fermée bloque ce déplacement"))
    } else {
        Ok(())
    }
}
fn radius(s: &Value, t: &Value) -> f64 {
    t["vision_cells"].as_f64().unwrap_or(6.).clamp(0., 60.) * s["grid"].as_f64().unwrap_or(50.)
}
fn visible(s: &Value, sources: &[Value], p: P) -> bool {
    sources.iter().any(|t| {
        let a = point(t, "x", "y");
        (p.0 - a.0).hypot(p.1 - a.1) <= radius(s, t) && !blocked(s, a, p, "blocks_sight")
    })
}
fn polygon(s: &Value, t: &Value) -> Value {
    let a = point(t, "x", "y");
    let r = radius(s, t);
    let mut angles: Vec<f64> = (0..128)
        .map(|i| i as f64 * std::f64::consts::TAU / 128.)
        .collect();
    let mut walls: Vec<(P, P)> = s["walls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| blocks(w, "blocks_sight"))
        .map(|w| (point(w, "x1", "y1"), point(w, "x2", "y2")))
        .collect();
    let width = s["width"].as_f64().unwrap_or(1600.);
    let height = s["height"].as_f64().unwrap_or(1000.);
    walls.extend([
        ((0., 0.), (width, 0.)),
        ((width, 0.), (width, height)),
        ((width, height), (0., height)),
        ((0., height), (0., 0.)),
    ]);
    for (c, d) in &walls {
        for p in [c, d] {
            let angle = (p.1 - a.1).atan2(p.0 - a.0);
            angles.extend([angle - 1e-5, angle, angle + 1e-5]);
        }
    }
    for angle in &mut angles {
        *angle = angle.rem_euclid(std::f64::consts::TAU);
    }
    angles.sort_by(f64::total_cmp);
    json!(
        angles
            .into_iter()
            .map(|angle| {
                let b = (a.0 + r * angle.cos(), a.1 + r * angle.sin());
                let f = walls
                    .iter()
                    .filter_map(|(c, d)| hit(a, b, *c, *d))
                    .fold(1., f64::min);
                json!([a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f])
            })
            .collect::<Vec<_>>()
    )
}
pub fn project(w: &mut Value, u: &User) {
    if u.role == "gm" {
        return;
    }
    if let Some(scenes) = w["scenes"].as_array_mut() {
        for s in scenes {
            if s["vision_enabled"] != true {
                continue;
            }
            let sources: Vec<Value> = s["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| owned(u, t) && t["hidden"] != true && radius(s, t) > 0.)
                .take(16)
                .cloned()
                .collect();
            let keep: Vec<Value> = s["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| owned(u, t) || visible(s, &sources, point(t, "x", "y")))
                .cloned()
                .collect();
            let notes: Vec<Value> = s["notes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| visible(s, &sources, point(n, "x", "y")))
                .cloned()
                .collect();
            s["vision_polygons"] = json!(sources.iter().map(|t| polygon(s, t)).collect::<Vec<_>>());
            s["tokens"] = json!(keep);
            s["notes"] = json!(notes);
        }
    }
}
pub fn validate(s: &Value) -> Result<(), (StatusCode, String)> {
    for key in ["collision", "vision_enabled"] {
        if s.get(key).is_some_and(|v| !v.is_boolean()) {
            return Err(bad("Option de scène invalide"));
        }
    }
    if s["walls"].as_array().is_some_and(|a| a.len() > 500) {
        return Err(bad("Limite : 500 murs par scène"));
    }
    for wall in s["walls"].as_array().ok_or(bad("Murs invalides"))? {
        for k in ["x1", "y1", "x2", "y2"] {
            num(wall, k, 0., 10000.)?;
        }
        if wall["door"] == true
            && !matches!(wall["state"].as_str(), Some("open" | "closed" | "locked"))
        {
            return Err(bad("État de porte invalide"));
        }
    }
    for token in s["tokens"].as_array().ok_or(bad("Pions invalides"))? {
        if token.get("vision_cells").is_some() {
            num(token, "vision_cells", 0., 60.)?;
        }
    }
    Ok(())
}
pub fn apply(w: &mut Value, u: &User, v: &Value) -> Result<bool, (StatusCode, String)> {
    let kind = v["kind"].as_str().unwrap_or("");
    if ![
        "scene_rules",
        "wall_add",
        "wall_update",
        "wall_delete",
        "door_toggle",
        "token_vision",
    ]
    .contains(&kind)
    {
        return Ok(false);
    }
    if u.role != "gm" && kind != "door_toggle" {
        return Err(forbidden());
    }
    let s = scene(w)?;
    match kind {
        "scene_rules" => {
            for key in ["collision", "vision_enabled"] {
                if let Some(value) = v.get(key) {
                    if !value.is_boolean() {
                        return Err(bad("Option booléenne requise"));
                    }
                    s[key] = value.clone();
                }
            }
        }
        "wall_add" => {
            let ws = s["walls"].as_array_mut().unwrap();
            if ws.len() >= 500 {
                return Err(bad("Limite : 500 murs par scène"));
            }
            let mut wall = json!({"id":id(),"door":v["door"]==true,"state":"closed","blocks_move":true,"blocks_sight":true});
            for k in ["x1", "y1", "x2", "y2"] {
                wall[k] = json!(num(v, k, 0., 10000.)?);
            }
            if point(&wall, "x1", "y1") == point(&wall, "x2", "y2") {
                return Err(bad("Un mur doit avoir une longueur"));
            }
            ws.push(wall);
        }
        "token_vision" => {
            let t = find(s["tokens"].as_array_mut().unwrap(), &v["id"])?;
            t["vision_cells"] = json!(num(v, "vision_cells", 0., 60.)?);
        }
        _ => {
            let index = v["index"]
                .as_u64()
                .filter(|n| *n < (s["walls"].as_array().unwrap().len() as u64))
                .ok_or(bad("Mur introuvable"))? as usize;
            // Geometry sent by UI prevents stale indices editing another wall after a concurrent deletion.
            let current = &s["walls"][index];
            for key in ["x1", "y1", "x2", "y2"] {
                if current[key].as_f64() != v["wall"][key].as_f64()
                    || current.get("id").is_some_and(|id| *id != v["wall"]["id"])
                {
                    return Err((
                        StatusCode::CONFLICT,
                        "Les murs ont changé, rouvrez leur gestion".into(),
                    ));
                }
            }
            if kind == "wall_delete" {
                s["walls"].as_array_mut().unwrap().remove(index);
                return Ok(true);
            }
            if kind == "door_toggle" {
                let door = &s["walls"][index];
                if door["door"] != true {
                    return Err(bad("Ce mur n’est pas une porte"));
                }
                if u.role != "gm" {
                    if door["state"] == "locked" {
                        return Err(forbidden());
                    }
                    let p = (
                        (door["x1"].as_f64().unwrap() + door["x2"].as_f64().unwrap()) / 2.,
                        (door["y1"].as_f64().unwrap() + door["y2"].as_f64().unwrap()) / 2.,
                    );
                    let close = s["tokens"].as_array().unwrap().iter().any(|t| {
                        owned(u, t) && t["hidden"] != true && {
                            let a = point(t, "x", "y");
                            (p.0 - a.0).hypot(p.1 - a.1) <= s["grid"].as_f64().unwrap_or(50.) * 1.6
                                && !covered(t, s["fog"].as_array().unwrap())
                        }
                    });
                    if !close {
                        return Err(bad("Approchez un de vos pions de la porte"));
                    }
                }
                s["walls"][index]["state"] = json!(if s["walls"][index]["state"] == "open" {
                    "closed"
                } else {
                    "open"
                });
            } else {
                let door = v["door"].as_bool().ok_or(bad("Type de mur requis"))?;
                let state = v["state"].as_str().unwrap_or("closed");
                if !["closed", "open", "locked"].contains(&state) {
                    return Err(bad("État de porte invalide"));
                }
                s["walls"][index]["door"] = json!(door);
                s["walls"][index]["state"] = json!(state);
                for key in ["blocks_move", "blocks_sight"] {
                    s["walls"][index][key] =
                        json!(v[key].as_bool().ok_or(bad("Blocage booléen requis"))?);
                }
            }
        }
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersections() {
        assert!(hit((0., 0.), (10., 0.), (5., -5.), (5., 5.)).is_some());
        assert!(hit((0., 0.), (10., 0.), (4., 0.), (8., 0.)).is_some());
        assert!(hit((0., 0.), (1., 0.), (2., -1.), (2., 1.)).is_none());
        assert!(hit((0., 0.), (5., 5.), (5., 5.), (5., 9.)).is_some());
    }
    #[test]
    fn sight_polygon_has_consistent_winding_across_zero_angle() {
        let mut w = demo();
        let s = &mut w["scenes"][0];
        s["walls"] =
            json!([{"x1":500,"y1":0,"x2":500,"y2":350},{"x1":500,"y1":450,"x2":500,"y2":1000}]);
        let source = json!({"x":450,"y":400,"vision_cells":10});
        let p = polygon(s, &source);
        let mut previous = -1.;
        for vertex in p.as_array().unwrap() {
            let x = vertex[0].as_f64().unwrap() - 450.;
            let y = vertex[1].as_f64().unwrap() - 400.;
            let angle = y.atan2(x).rem_euclid(std::f64::consts::TAU);
            assert!(
                angle + 1e-8 >= previous,
                "polygon crosses itself at an angle wrap"
            );
            previous = angle;
        }
    }
    #[test]
    fn doors_and_visibility() {
        let mut w = demo();
        let u = User {
            name: "Alice".into(),
            role: "player".into(),
        };
        let gm = User {
            name: "MJ".into(),
            role: "gm".into(),
        };
        let s = &mut w["scenes"][0];
        s["collision"] = json!(true);
        s["vision_enabled"] = json!(true);
        s["walls"] = json!([{"x1":500,"y1":0,"x2":500,"y2":1000,"door":true,"state":"closed"}]);
        assert!(check_move(s, &s["tokens"][0], (650., 400.), &u).is_err());
        assert!(check_move(s, &s["tokens"][0], (650., 400.), &gm).is_ok());
        let mut hidden = w.clone();
        project(&mut hidden, &u);
        assert_eq!(hidden["scenes"][0]["tokens"].as_array().unwrap().len(), 1);
        w["scenes"][0]["walls"][0]["state"] = json!("open");
        w["scenes"][0]["tokens"][0]["vision_cells"] = json!(20);
        project(&mut w, &u);
        assert_eq!(w["scenes"][0]["tokens"].as_array().unwrap().len(), 2);
        assert!(
            !w["scenes"][0]["vision_polygons"][0]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
