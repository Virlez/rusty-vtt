use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        Html, IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};
use tokio::sync::{Mutex, Semaphore, broadcast};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
mod builder;
mod chat;
mod effects;
mod modules;
mod scene_tools;
mod security;

#[derive(Clone, Serialize, Deserialize)]
struct User {
    name: String,
    role: String,
}
struct Inner {
    world: Value,
    revision: u64,
    sessions: HashMap<String, security::Session>,
    logins: HashMap<IpAddr, security::Window>,
}
#[derive(Clone)]
struct App {
    inner: Arc<Mutex<Inner>>,
    events: broadcast::Sender<u64>,
    file: PathBuf,
    gm_password: Arc<String>,
    players: Arc<HashMap<String, String>>,
    secure_cookies: bool,
    streams: Arc<Semaphore>,
}
type ApiResult = Result<Json<Value>, (StatusCode, String)>;
fn bad(s: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, s.into())
}
fn forbidden() -> (StatusCode, String) {
    (
        StatusCode::FORBIDDEN,
        "Action réservée au MJ ou au propriétaire".into(),
    )
}
fn id() -> String {
    format!("{:032x}", rand::rng().random::<u128>())
}
fn user(headers: &HeaderMap, inner: &Inner) -> Result<User, (StatusCode, String)> {
    session_key(headers)
        .and_then(|k| inner.sessions.get(k))
        .filter(|s| s.expires > Instant::now())
        .map(|s| s.user.clone())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Session expirée ou absente. Reconnectez-vous.".into(),
        ))
}
fn session_key(headers: &HeaderMap) -> Option<&str> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    cookie
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("rusty_session="))
}
fn snapshot(w: &Value, u: &User, revision: u64) -> Value {
    let source = w;
    let mut w = w.clone();
    chat::project(&mut w, u);
    if u.role != "gm" {
        if let Some(drafts) = w["builder_drafts"].as_array_mut() {
            drafts.retain(|d| d["owner"] == u.name);
        }
        if let Some(scenes) = w["scenes"].as_array_mut() {
            for scene in scenes {
                let fog = scene["fog"].as_array().cloned().unwrap_or_default();
                if let Some(tokens) = scene["tokens"].as_array_mut() {
                    tokens.retain(|t| t["hidden"] != true && !covered(t, &fog));
                }
                if let Some(notes) = scene["notes"].as_array_mut() {
                    notes.retain(|n| !covered(n, &fog));
                }
            }
        }
        if let Some(j) = w["journals"].as_array_mut() {
            j.retain(|j| j["public"] == true);
        }
        if let Some(a) = w["actors"].as_array_mut() {
            for actor in a {
                if !owned(u, actor) {
                    actor["notes"] = json!("");
                    if actor.get("foundry").is_some() {
                        actor.as_object_mut().unwrap().remove("foundry");
                        actor["items"] = json!([]);
                    }
                    if let Some(dnd5e) = actor["dnd5e"].as_object_mut() {
                        dnd5e.remove("build");
                    }
                }
            }
        }
        for key in ["items", "tables"] {
            if let Some(entries) = w[key].as_array_mut() {
                entries.retain(|e| e["public"] != false);
            }
        }
    }
    scene_tools::project(&mut w, u);
    if u.role != "gm" {
        // Remove combat references to tokens absent after fog/vision projection.
        let token_ids = |world: &Value| -> Vec<Value> {
            world["scenes"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|s| s["tokens"].as_array().into_iter().flatten())
                .map(|t| t["id"].clone())
                .collect()
        };
        let all = token_ids(source);
        let shown = token_ids(&w);
        let original = w["combat"]["order"].as_array().cloned().unwrap_or_default();
        let current = w["combat"]["turn"].as_u64().unwrap_or(0) as usize;
        let visible = |entry: &&Value| !all.contains(&entry["id"]) || shown.contains(&entry["id"]);
        let order: Vec<Value> = original.iter().filter(visible).cloned().collect();
        let next = (0..original.len())
            .map(|n| &original[(current + n) % original.len()])
            .find(|entry| visible(entry));
        let turn = next
            .and_then(|entry| order.iter().position(|v| v == entry))
            .unwrap_or(0);
        w["combat"]["order"] = json!(order);
        w["combat"]["turn"] = json!(turn);
    }
    let installed_modules = modules::manifest(&w, u);
    json!({"world":w,"user":u,"revision":revision,"installed_modules":installed_modules})
}
fn covered(point: &Value, fog: &[Value]) -> bool {
    let x = point["x"].as_f64().unwrap_or(0.);
    let y = point["y"].as_f64().unwrap_or(0.);
    fog.iter().any(|r| {
        let left = r["x"].as_f64().unwrap_or(0.);
        let top = r["y"].as_f64().unwrap_or(0.);
        x >= left
            && x <= left + r["w"].as_f64().unwrap_or(0.)
            && y >= top
            && y <= top + r["h"].as_f64().unwrap_or(0.)
    })
}
fn demo() -> Value {
    json!({"name":"Les Chroniques de l’Ambre","active_scene":"scene-camp","scenes":[{"id":"scene-camp","name":"Le camp des aventuriers","width":1600,"height":1000,"grid":50,"background":"","tokens":[{"id":"hero","name":"Ariane","x":400,"y":400,"color":"#e5b76e","hp":24,"max_hp":24,"owner":"","hidden":false},{"id":"wolf","name":"Loup des brumes","x":750,"y":350,"color":"#9caec4","hp":12,"max_hp":12,"owner":"MJ","hidden":false}],"walls":[],"fog":[],"notes":[]}],"actors":[{"id":"hero","name":"Ariane","hp":24,"max_hp":24,"armor":15,"initiative":2,"owner":"","notes":"Rôdeuse des marches du Nord.","items":[]}],"journals":[{"id":"welcome","name":"Bienvenue aux marches du Nord","content":"Une lueur étrange brille au-delà des ruines. Rassemblez vos compagnons et décidez de votre prochaine aventure.\n\nLe MJ peut préparer des scènes, déplacer les adversaires et dévoiler la carte.","public":true}],"items":[],"combat":{"order":[],"turn":0,"round":1},"messages":[],"playlist":[]})
}
async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}
async fn advanced_asset(axum::extract::Path(name): axum::extract::Path<String>) -> Response {
    let (mime, source) = match name.as_str() {
        "scene-ui.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../web/scene-ui.js"),
        ),
        "chat-ui.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../web/chat-ui.js"),
        ),
        "effects-ui.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../web/effects-ui.js"),
        ),
        "advanced.css" => (
            "text/css; charset=utf-8",
            include_str!("../web/advanced.css"),
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        source,
    )
        .into_response()
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/style.css"),
    )
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/app.js"),
    )
}
async fn modules_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/modules-ui.js"),
    )
}
async fn builder_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/builder-ui.js"),
    )
}
async fn login(
    State(app): State<App>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(v): Json<Value>,
) -> Result<Response, (StatusCode, String)> {
    let name = v["name"].as_str().unwrap_or("").trim();
    if name.is_empty() || name.len() > 60 {
        return Err(bad("Nom requis (60 caractères maximum)"));
    }
    let mut inner = app.inner.lock().await;
    inner.sessions.retain(|_, s| s.expires > Instant::now());
    inner.logins.retain(|_, w| !w.expired());
    if inner.logins.len() >= 1024 && !inner.logins.contains_key(&peer.ip()) {
        return Err(security::throttled());
    }
    if !inner
        .logins
        .entry(peer.ip())
        .or_insert_with(security::Window::new)
        .allow(30)
    {
        return Err(security::throttled());
    }
    let pw = v["password"].as_str().unwrap_or("");
    let role = if pw == app.gm_password.as_str() {
        "gm"
    } else if app.players.get(name).is_some_and(|secret| secret == pw) {
        "player"
    } else {
        return Err((StatusCode::UNAUTHORIZED, "Nom ou secret incorrect".into()));
    };
    let u = User {
        name: name.into(),
        role: role.into(),
    };
    // Rotate the current session only after credentials have been checked.
    if let Some(key) = session_key(&headers) {
        inner.sessions.remove(key);
    }
    if inner.sessions.len() >= security::MAX_SESSIONS
        || inner
            .sessions
            .values()
            .filter(|s| s.user.name == name)
            .count()
            >= security::MAX_USER_SESSIONS
    {
        return Err(security::throttled());
    }
    let token = id();
    inner
        .sessions
        .insert(token.clone(), security::Session::new(u.clone()));
    Ok((
        [(
            header::SET_COOKIE,
            session_cookie(&token, app.secure_cookies, 8 * 60 * 60),
        )],
        Json(snapshot(&inner.world, &u, inner.revision)),
    )
        .into_response())
}
fn session_cookie(token: &str, secure: bool, age: u64) -> String {
    format!(
        "rusty_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={age}{}",
        if secure { "; Secure" } else { "" }
    )
}
async fn transport(State(app): State<App>, request: Request, next: Next) -> Response {
    let https = request
        .headers()
        .get("x-forwarded-proto")
        .is_some_and(|v| v == "https");
    // Only a local reverse proxy can reach this listener; never trust this header
    // on a public bind. The proxy must overwrite it and redirect HTTP to HTTPS.
    if app.secure_cookies && request.uri().path().starts_with("/api/") && !https {
        return (
            StatusCode::UPGRADE_REQUIRED,
            "HTTPS requis via le proxy local",
        )
            .into_response();
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if app.secure_cookies && https {
        response.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            header::HeaderValue::from_static("max-age=31536000"),
        );
    }
    response
}
async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    let mut inner = app.inner.lock().await;
    if let Some(key) = session_key(&headers) {
        inner.sessions.remove(key);
    }
    (
        [(
            header::SET_COOKIE,
            session_cookie("", app.secure_cookies, 0),
        )],
        Json(json!({"ok":true})),
    )
        .into_response()
}
async fn state(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    let i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    Ok(Json(snapshot(&i.world, &u, i.revision)))
}
async fn export(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    let i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    if u.role != "gm" {
        return Err(forbidden());
    }
    Ok(Json(i.world.clone()))
}
async fn persist(app: &App, w: &Value) -> Result<(), (StatusCode, String)> {
    let bytes = serde_json::to_vec_pretty(w).map_err(|_| bad("Sauvegarde invalide"))?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(bad("Le monde dépasse la limite de 32 Mio"));
    }
    let tmp = app.file.with_extension("tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tokio::fs::rename(tmp, &app.file)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
fn validate_world(w: &Value) -> Result<(), (StatusCode, String)> {
    effects::validate(w)?;
    if let Some(m) = w.get("modules")
        && (!m.is_object()
            || m.as_object()
                .unwrap()
                .values()
                .any(|v| !v["enabled"].is_boolean()))
    {
        return Err(bad("Configuration des modules invalide"));
    }
    if let Some(t) = w.get("tables")
        && (!t.is_array()
            || t.as_array()
                .unwrap()
                .iter()
                .any(|d| !d.is_object() || !d["id"].is_string()))
    {
        return Err(bad("Tables invalides"));
    }
    if let Some(d) = w.get("builder_drafts")
        && (!d.is_array()
            || d.as_array().unwrap().iter().any(|x| {
                !x.is_object()
                    || !x["id"].is_string()
                    || !x["owner"].is_string()
                    || !x["data"].is_object()
            }))
    {
        return Err(bad("Brouillons de personnage invalides"));
    }
    if !w["name"].is_string() || !w["active_scene"].is_string() {
        return Err(bad("Monde invalide"));
    }
    for k in [
        "scenes", "actors", "journals", "items", "messages", "playlist",
    ] {
        if !w[k].is_array() {
            return Err(bad("Collections du monde invalides"));
        }
        if w[k]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| !entry.is_object() || !entry["id"].is_string())
        {
            return Err(bad("Chaque document doit avoir un identifiant texte"));
        }
    }
    if !w["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == w["active_scene"])
    {
        return Err(bad("Scène active absente"));
    }
    for actor in w["actors"].as_array().unwrap() {
        if actor.get("dnd5e").is_some_and(|d| !d.is_object()) {
            return Err(bad("Données 5e invalides"));
        }
        if actor.get("max_hp").is_some() {
            max_hp(actor)?;
        }
    }
    for s in w["scenes"].as_array().unwrap() {
        scene_tools::validate(s)?;
        num(s, "width", 200., 10000.)?;
        num(s, "height", 200., 10000.)?;
        text(s, "name", 100)?;
        for k in ["tokens", "walls", "fog", "notes"] {
            if !s[k].is_array() {
                return Err(bad("Scène invalide"));
            }
            if s[k].as_array().unwrap().iter().any(|e| !e.is_object()) {
                return Err(bad("Document de scène invalide"));
            }
        }
        for t in s["tokens"].as_array().unwrap() {
            text(t, "id", 100)?;
            text(t, "name", 100)?;
            num(t, "x", 0., 10000.)?;
            num(t, "y", 0., 10000.)?;
            for key in ["owner", "color"] {
                if t.get(key).is_some_and(|value| !value.is_string()) {
                    return Err(bad("Texte de pion invalide"));
                }
            }
            if t.get("hidden").is_some_and(|value| !value.is_boolean()) {
                return Err(bad("Visibilité de pion invalide"));
            }
        }
        if !s["grid"]
            .as_f64()
            .is_some_and(|n| (10.0..=300.0).contains(&n))
        {
            return Err(bad("Grille invalide"));
        }
    }
    if !w["combat"]["order"].is_array() {
        return Err(bad("Combat invalide"));
    }
    if w["combat"]["order"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| !e.is_object() || !e["id"].is_string())
    {
        return Err(bad("Combattant invalide"));
    }
    num(&w["combat"], "turn", 0., 10000.)?;
    num(&w["combat"], "round", 1., 1000000.)?;
    Ok(())
}
async fn import(State(app): State<App>, headers: HeaderMap, Json(w): Json<Value>) -> ApiResult {
    let mut i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    if u.role != "gm" {
        return Err(forbidden());
    }
    validate_world(&w)?;
    persist(&app, &w).await?;
    i.world = w;
    i.revision += 1;
    let _ = app.events.send(i.revision);
    Ok(Json(snapshot(&i.world, &u, i.revision)))
}

#[derive(Debug, Serialize)]
struct Roll {
    formula: String,
    total: i64,
    detail: String,
}
fn roll(formula: &str) -> Result<Roll, (StatusCode, String)> {
    let f = formula.to_lowercase().replace(' ', "");
    if f.len() > 80 {
        return Err(bad("Formule trop longue"));
    }
    let mut total = 0i64;
    let mut details = vec![];
    let bytes = f.as_bytes();
    let mut pos = 0;
    let mut dice_count = 0u32;
    while pos < bytes.len() {
        let sign = if bytes[pos] == b'-' {
            pos += 1;
            -1
        } else {
            if bytes[pos] == b'+' {
                pos += 1;
            }
            1
        };
        let start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        let digits = &f[start..pos];
        if pos < bytes.len() && bytes[pos] == b'd' {
            pos += 1;
            let count = if digits.is_empty() {
                1
            } else {
                digits
                    .parse::<u32>()
                    .map_err(|_| bad("Nombre de dés invalide"))?
            };
            let start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            let sides = f[start..pos]
                .parse::<u32>()
                .map_err(|_| bad("Faces manquantes"))?;
            dice_count = dice_count.checked_add(count).ok_or(bad("Trop de dés"))?;
            if count == 0 || dice_count > 100 || !(2..=100000).contains(&sides) {
                return Err(bad("Limites : 100 dés, 2 à 100000 faces"));
            }
            let results: Vec<u32> = (0..count)
                .map(|_| rand::rng().random_range(1..=sides))
                .collect();
            total += sign * results.iter().map(|n| *n as i64).sum::<i64>();
            details.push(format!("{sign:+} {count}d{sides}: {results:?}"));
        } else {
            if digits.is_empty() {
                return Err(bad("Formule attendue : 1d20+5 ou 2d6-1"));
            }
            let n = digits
                .parse::<i64>()
                .map_err(|_| bad("Modificateur invalide"))?;
            if n > 1000000 {
                return Err(bad("Modificateur trop grand"));
            }
            total += sign * n;
            details.push(format!("{:+}", sign * n));
        }
        if pos < bytes.len() && bytes[pos] != b'+' && bytes[pos] != b'-' {
            return Err(bad("Opérateur invalide"));
        }
    }
    if f.is_empty() {
        return Err(bad("Formule requise"));
    }
    Ok(Roll {
        formula: f,
        total,
        detail: details.join(" ; "),
    })
}
fn text(v: &Value, key: &str, max: usize) -> Result<String, (StatusCode, String)> {
    let s = v[key].as_str().unwrap_or("").trim();
    if s.is_empty() || s.len() > max {
        return Err(bad(&format!("Champ {key} requis (maximum {max})")));
    }
    Ok(s.into())
}
fn num(v: &Value, key: &str, min: f64, max: f64) -> Result<f64, (StatusCode, String)> {
    let n = v[key]
        .as_f64()
        .ok_or(bad(&format!("Nombre {key} requis")))?;
    if !n.is_finite() || n < min || n > max {
        return Err(bad(&format!("{key} hors limites")));
    }
    Ok(n)
}
fn max_hp(v: &Value) -> Result<i64, (StatusCode, String)> {
    let value = num(v, "max_hp", 1., 10000.)?;
    if value.fract() != 0. {
        return Err(bad("Le maximum de PV doit être entier"));
    }
    Ok(value as i64)
}
fn owned(u: &User, v: &Value) -> bool {
    u.role == "gm" || v["owner"] == u.name || v["owner"] == ""
}
fn collection<'a>(w: &'a mut Value, k: &str) -> Result<&'a mut Vec<Value>, (StatusCode, String)> {
    w[k].as_array_mut().ok_or(bad("Collection invalide"))
}
fn scene(w: &mut Value) -> Result<&mut Value, (StatusCode, String)> {
    let active = w["active_scene"].clone();
    w["scenes"]
        .as_array_mut()
        .and_then(|s| s.iter_mut().find(|s| s["id"] == active))
        .ok_or(bad("Scène introuvable"))
}
fn find<'a>(list: &'a mut [Value], id: &Value) -> Result<&'a mut Value, (StatusCode, String)> {
    list.iter_mut()
        .find(|x| x["id"] == *id)
        .ok_or(bad("Élément introuvable"))
}
fn apply(w: &mut Value, u: &User, v: &Value) -> Result<(), (StatusCode, String)> {
    let kind = v["kind"].as_str().ok_or(bad("Action requise"))?;
    if chat::apply(w, u, v)? || scene_tools::apply(w, u, v)? || effects::apply(w, u, v)? {
        return Ok(());
    }
    if !["chat", "roll", "move_token", "update_actor", "update_token"].contains(&kind)
        && u.role != "gm"
    {
        return Err(forbidden());
    }
    match kind {
        "chat" | "roll" => {
            let (message, r) = if kind == "roll" {
                let r = roll(&text(v, "formula", 80)?)?;
                (
                    format!("{} = {}", r.formula, r.total),
                    serde_json::to_value(r).unwrap(),
                )
            } else {
                (text(v, "text", 4000)?, Value::Null)
            };
            let m = collection(w, "messages")?;
            m.push(json!({"id":id(),"author":u.name,"text":message,"roll":r}));
            if m.len() > 500 {
                m.remove(0);
            }
        }
        "move_token" => {
            let s = scene(w)?;
            let width = s["width"].as_f64().unwrap_or(1600.);
            let height = s["height"].as_f64().unwrap_or(1000.);
            let proposed = (num(v, "x", 0., width)?, num(v, "y", 0., height)?);
            let candidate = s["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["id"] == v["id"])
                .ok_or(bad("Pion introuvable"))?;
            if !owned(u, candidate) || candidate["hidden"] == true && u.role != "gm" {
                return Err(forbidden());
            }
            scene_tools::check_move(s, candidate, proposed, u)?;
            let t = find(s["tokens"].as_array_mut().unwrap(), &v["id"])?;
            if !owned(u, t) || t["hidden"] == true && u.role != "gm" {
                return Err(forbidden());
            }
            t["x"] = json!(num(v, "x", 0., width)?);
            t["y"] = json!(num(v, "y", 0., height)?);
        }
        "add_token" => {
            let s = scene(w)?;
            let name = text(v, "name", 100)?;
            let x = num(v, "x", 0., s["width"].as_f64().unwrap_or(1600.))?;
            let y = num(v, "y", 0., s["height"].as_f64().unwrap_or(1000.))?;
            s["tokens"].as_array_mut().unwrap().push(json!({"id":id(),"name":name,"x":x,"y":y,"color":v["color"].as_str().unwrap_or("#e5b76e"),"hp":20,"max_hp":20,"owner":v["owner"].as_str().unwrap_or(""),"hidden":false}));
        }
        "update_token" => {
            let t = find(scene(w)?["tokens"].as_array_mut().unwrap(), &v["id"])?;
            if !owned(u, t) || t["hidden"] == true && u.role != "gm" {
                return Err(forbidden());
            }
            if v.get("hp").is_some() {
                t["hp"] = json!(num(v, "hp", -10000., 10000.)?);
            }
            if u.role == "gm" {
                for k in ["name", "owner", "color", "hidden"] {
                    if let Some(val) = v.get(k) {
                        match k {
                            "name" => {
                                text(v, k, 100)?;
                            }
                            "hidden" if !val.is_boolean() => {
                                return Err(bad("Visibilité de pion invalide"));
                            }
                            "owner" | "color" if !val.is_string() => {
                                return Err(bad("Texte de pion invalide"));
                            }
                            _ => {}
                        }
                        t[k] = val.clone();
                    }
                }
            }
        }
        "delete_token" => {
            scene(w)?["tokens"]
                .as_array_mut()
                .unwrap()
                .retain(|t| t["id"] != v["id"]);
        }
        "add_actor" => {
            let name = text(v, "name", 100)?;
            let hp = v["hp"].as_i64().unwrap_or(20).clamp(0, 10000);
            collection(w,"actors")?.push(json!({"id":id(),"name":name,"hp":hp,"max_hp":v["max_hp"].as_i64().unwrap_or(hp).clamp(1,10000),"armor":v["armor"].as_i64().unwrap_or(10),"initiative":0,"owner":v["owner"].as_str().unwrap_or(""),"notes":v["notes"].as_str().unwrap_or(""),"items":[]}));
        }
        "update_actor" => {
            let a = find(collection(w, "actors")?, &v["id"])?;
            if !owned(u, a) {
                return Err(forbidden());
            }
            for k in ["name", "notes"] {
                if let Some(val) = v.get(k) {
                    if !val.is_string() || val.as_str().unwrap().len() > 20000 {
                        return Err(bad("Texte invalide"));
                    }
                    a[k] = val.clone();
                }
            }
            if v.get("max_hp").is_some() {
                a["max_hp"] = json!(max_hp(v)?);
            }
            for k in ["hp", "armor", "initiative"] {
                if v.get(k).is_some() {
                    a[k] = json!(num(v, k, -10000., 10000.)?);
                }
            }
            if u.role == "gm"
                && let Some(o) = v.get("owner")
            {
                a["owner"] = o.clone();
            }
        }
        "add_journal" => {
            let name = text(v, "name", 100)?;
            let content = text(v, "content", 50000)?;
            collection(w,"journals")?.push(json!({"id":id(),"name":name,"content":content,"public":v["public"].as_bool().unwrap_or(false)}));
        }
        "add_item" => {
            let name = text(v, "name", 100)?;
            collection(w,"items")?.push(json!({"id":id(),"name":name,"description":v["description"].as_str().unwrap_or("")}));
        }
        "add_scene" => {
            let name = text(v, "name", 100)?;
            collection(w,"scenes")?.push(json!({"id":id(),"name":name,"width":v["width"].as_u64().unwrap_or(1600).clamp(200,10000),"height":v["height"].as_u64().unwrap_or(1000).clamp(200,10000),"grid":v["grid"].as_u64().unwrap_or(50).clamp(10,300),"background":v["background"].as_str().unwrap_or(""),"tokens":[],"walls":[],"fog":[],"notes":[]}));
        }
        "activate_scene" => {
            if !w["scenes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["id"] == v["id"])
            {
                return Err(bad("Scène introuvable"));
            }
            w["active_scene"] = v["id"].clone();
        }
        "update_scene" => {
            let s = scene(w)?;
            if v.get("grid").is_some() {
                s["grid"] = json!(num(v, "grid", 10., 300.)?);
            }
            if let Some(b) = v.get("background") {
                s["background"] = b.clone();
            }
        }
        "combat_add" => {
            let candidate = w["actors"]
                .as_array()
                .unwrap()
                .iter()
                .chain(
                    w["scenes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(|s| s["tokens"].as_array().unwrap().iter()),
                )
                .find(|a| a["id"] == v["id"])
                .cloned()
                .ok_or(bad("Combattant introuvable"))?;
            let order = w["combat"]["order"].as_array_mut().unwrap();
            if !order.iter().any(|a| a["id"] == candidate["id"]) {
                order.push(json!({"id":candidate["id"],"name":candidate["name"],"initiative":rand::rng().random_range(1..=20)+candidate["initiative"].as_i64().unwrap_or(0)}));
                order.sort_by_key(|a| -a["initiative"].as_i64().unwrap_or(0));
            }
        }
        "combat_next" => {
            let len = w["combat"]["order"].as_array().unwrap().len();
            if len > 0 {
                let next = w["combat"]["turn"].as_u64().unwrap_or(0) + 1;
                w["combat"]["turn"] = json!(next % len as u64);
                if next >= len as u64 {
                    w["combat"]["round"] = json!(w["combat"]["round"].as_u64().unwrap_or(1) + 1);
                    effects::advance_round(w);
                }
            }
        }
        "combat_reset" => w["combat"] = json!({"order":[],"turn":0,"round":1}),
        "fog_add" => {
            let rect = json!({"x":num(v,"x",0.,10000.)?,"y":num(v,"y",0.,10000.)?,"w":num(v,"w",1.,10000.)?,"h":num(v,"h",1.,10000.)?});
            scene(w)?["fog"].as_array_mut().unwrap().push(rect);
        }
        "fog_clear" => scene(w)?["fog"] = json!([]),
        "wall_add" => {
            let wall = json!({"x1":num(v,"x1",0.,10000.)?,"y1":num(v,"y1",0.,10000.)?,"x2":num(v,"x2",0.,10000.)?,"y2":num(v,"y2",0.,10000.)?});
            scene(w)?["walls"].as_array_mut().unwrap().push(wall);
        }
        "walls_clear" => scene(w)?["walls"] = json!([]),
        "note_add" => {
            let note = json!({"x":num(v,"x",0.,10000.)?,"y":num(v,"y",0.,10000.)?,"text":text(v,"text",2000)?});
            scene(w)?["notes"].as_array_mut().unwrap().push(note);
        }
        "playlist_add" => {
            let name = text(v, "name", 100)?;
            let url = text(v, "url", 4000)?;
            if !url.starts_with("https://") && !url.starts_with("http://") {
                return Err(bad("URL audio HTTP(S) requise"));
            }
            collection(w, "playlist")?.push(json!({"id":id(),"name":name,"url":url}));
        }
        _ => return Err(bad("Action inconnue")),
    }
    Ok(())
}
async fn action(State(app): State<App>, headers: HeaderMap, Json(v): Json<Value>) -> ApiResult {
    let mut i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    if !i
        .sessions
        .get_mut(session_key(&headers).unwrap())
        .unwrap()
        .actions
        .allow(120)
    {
        return Err(security::throttled());
    }
    let mut next = i.world.clone();
    if !builder::apply(&mut next, &u, &v)? && !modules::apply(&mut next, &u, &v)? {
        apply(&mut next, &u, &v)?;
    }
    validate_world(&next)?;
    persist(&app, &next).await?;
    i.world = next;
    i.revision += 1;
    let _ = app.events.send(i.revision);
    Ok(Json(snapshot(&i.world, &u, i.revision)))
}
async fn event_stream(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let i = app.inner.lock().await;
    user(&headers, &i)?;
    let session_permit = i
        .sessions
        .get(session_key(&headers).unwrap())
        .unwrap()
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| security::throttled())?;
    let permit = app
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| security::throttled())?;
    let rx = app.events.subscribe();
    drop(i);
    let stream = BroadcastStream::new(rx).map(move |r| {
        let _hold_permit = &permit;
        let _hold_session_permit = &session_permit;
        Ok::<_, Infallible>(
            Event::default().data(r.map(|n| n.to_string()).unwrap_or("resync".into())),
        )
    });
    let end = async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if user(&headers, &*app.inner.lock().await).is_err() {
                break;
            }
        }
    };
    let stream = futures_util::StreamExt::take_until(stream, end);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
#[tokio::main]
async fn main() {
    let file = PathBuf::from(std::env::var("RUSTY_DATA").unwrap_or("data/world.json".into()));
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .expect("Création dossier de données");
    }
    let world = match tokio::fs::read(&file).await {
        Ok(bytes) => {
            let w: Value = serde_json::from_slice(&bytes)
                .expect("Sauvegarde JSON invalide : restaurer une copie");
            validate_world(&w).expect("Structure de sauvegarde invalide");
            w
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => demo(),
        Err(e) => panic!("Lecture sauvegarde : {e}"),
    };
    let (events, _) = broadcast::channel(64);
    let password = std::env::var("RUSTY_GM_PASSWORD").unwrap_or_else(|_| {
        let generated = id();
        println!("Secret MJ généré pour cette exécution : {generated}");
        generated
    });
    assert!(
        password.len() >= 16,
        "Le secret MJ doit avoir au moins 16 octets"
    );
    let players = security::players(
        &std::env::var("RUSTY_PLAYERS_JSON").unwrap_or("{}".into()),
        &password,
    )
    .expect("Configuration des joueurs invalide");
    let addr =
        security::bind_address(&std::env::var("RUSTY_BIND").unwrap_or("127.0.0.1:3000".into()))
            .expect("Adresse HTTP non sécurisée");
    let secure_cookies = match std::env::var("RUSTY_HTTPS").as_deref() {
        Ok("1") => true,
        Err(std::env::VarError::NotPresent) | Ok("0") => false,
        _ => panic!("RUSTY_HTTPS doit valoir 0 ou 1"),
    };
    let app = App {
        inner: Arc::new(Mutex::new(Inner {
            world,
            revision: 0,
            sessions: HashMap::new(),
            logins: HashMap::new(),
        })),
        events,
        file,
        gm_password: Arc::new(password),
        players: Arc::new(players),
        secure_cookies,
        streams: Arc::new(Semaphore::new(32)),
    };
    let router = Router::new()
        .route("/", get(index))
        .route("/advanced/{name}", get(advanced_asset))
        .route("/style.css", get(css))
        .route("/app.js", get(js))
        .route("/modules-ui.js", get(modules_js))
        .route("/builder-ui.js", get(builder_js))
        .route("/api/login", post(login).layer(DefaultBodyLimit::max(4096)))
        .route("/api/logout", post(logout))
        .route("/api/state", get(state))
        .route("/api/action", post(action))
        .route("/api/events", get(event_stream))
        .route("/api/export", get(export))
        .route("/api/import", post(import))
        .route("/api/modules", get(modules::list_modules))
        .route("/api/builder/catalog", get(builder::get_catalog))
        .route("/api/builder/reference/{page}", get(builder::reference))
        .route("/api/builder/preview/{draft}", get(builder::preview))
        .route("/api/compendiums", get(modules::list_packs))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), transport))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Adresse indisponible");
    let addr = listener.local_addr().expect("Adresse locale indisponible");
    println!("Rusty VTT — http://{addr} — Ctrl+C pour arrêter");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fog_includes_edges_but_not_adjacent_points() {
        let fog = vec![json!({"x":10,"y":20,"w":30,"h":40})];
        for (x, y) in [(10., 20.), (40., 60.), (25., 40.)] {
            assert!(covered(&json!({"x":x,"y":y}), &fog));
        }
        for (x, y) in [(9.99, 20.), (40.01, 60.), (25., 60.01), (25., 19.99)] {
            assert!(!covered(&json!({"x":x,"y":y}), &fog));
        }
        assert!(!covered(&json!({"x":10,"y":20}), &[]));
    }

    #[test]
    fn fixed_dice_arithmetic_and_detail_agree() {
        for (formula, total) in [("42", 42), ("10+5-3", 12), ("2d6-2d6", 0)] {
            let result = roll(formula).unwrap();
            if !formula.contains('d') {
                assert_eq!(result.total, total);
            } else {
                assert!((-10..=10).contains(&result.total));
            }
            assert!(!result.formula.is_empty());
        }
    }
    fn player() -> User {
        User {
            name: "Alice".into(),
            role: "player".into(),
        }
    }
    #[test]
    fn expired_sessions_are_rejected_and_streams_bounded() {
        let mut inner = Inner {
            world: demo(),
            revision: 0,
            sessions: HashMap::new(),
            logins: HashMap::new(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "rusty_session=test".parse().unwrap());
        let mut session = security::Session::new(player());
        let first = session.streams.clone().try_acquire_owned().unwrap();
        let second = session.streams.clone().try_acquire_owned().unwrap();
        assert!(session.streams.clone().try_acquire_owned().is_err());
        drop((first, second));
        assert!(session.streams.clone().try_acquire_owned().is_ok());
        session.expires = Instant::now() - std::time::Duration::from_secs(1);
        inner.sessions.insert("test".into(), session);
        assert_eq!(
            user(&headers, &inner).err().unwrap().0,
            StatusCode::UNAUTHORIZED
        );
    }
    #[test]
    fn combat_projection_remaps_turns_and_hides_vision_targets() {
        let mut w = demo();
        w["combat"]["order"] = json!([{"id":"wolf","name":"Secret","initiative":20},{"id":"hero","name":"Hero","initiative":10}]);
        w["combat"]["turn"] = json!(0);
        w["scenes"][0]["tokens"][1]["hidden"] = json!(true);
        let view = snapshot(&w, &player(), 0);
        assert_eq!(
            view["world"]["combat"]["order"].as_array().unwrap().len(),
            1
        );
        assert_eq!(view["world"]["combat"]["turn"], 0);
        w["scenes"][0]["tokens"][1]["hidden"] = json!(false);
        w["scenes"][0]["vision_enabled"] = json!(true);
        w["scenes"][0]["tokens"][0]["vision_cells"] = json!(1);
        assert!(
            !snapshot(&w, &player(), 0)["world"]["combat"]["order"]
                .to_string()
                .contains("Secret")
        );
        w["scenes"][0]["tokens"][0]["hidden"] = json!(true);
        let view = snapshot(&w, &player(), 0);
        assert_eq!(view["world"]["combat"]["order"], json!([]));
        assert_eq!(view["world"]["combat"]["turn"], 0);
    }
    #[test]
    fn dice_boundaries() {
        for _ in 0..100 {
            let r = roll("2d6+3-1").unwrap();
            assert!((4..=14).contains(&r.total));
        }
        for f in [
            "",
            "0d6",
            "101d6",
            "d1",
            "d6/2",
            "d6+",
            "99999999999999999999999",
            "é",
        ] {
            assert!(roll(f).is_err(), "{f}");
        }
    }
    #[test]
    fn permission_and_hidden_projection() {
        let mut w = demo();
        let p = player();
        assert!(apply(&mut w, &p, &json!({"kind":"fog_clear"})).is_err());
        assert!(
            apply(
                &mut w,
                &p,
                &json!({"kind":"move_token","id":"wolf","x":0,"y":0})
            )
            .is_err()
        );
        w["scenes"][0]["tokens"][1]["hidden"] = json!(true);
        w["journals"]
            .as_array_mut()
            .unwrap()
            .push(json!({"public":false,"content":"secret"}));
        let s = snapshot(&w, &p, 0);
        assert_eq!(
            s["world"]["scenes"][0]["tokens"].as_array().unwrap().len(),
            1
        );
        assert_eq!(s["world"]["journals"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn malformed_5e_projection_is_defensive() {
        let mut w = demo();
        w["actors"][0]["owner"] = json!("MJ");
        for value in [json!(42), json!(null), json!([])] {
            w["actors"][0]["dnd5e"] = value;
            assert!(validate_world(&w).is_err());
            let projected = snapshot(&w, &player(), 0);
            assert_eq!(projected["world"]["actors"][0]["notes"], "");
        }
        w["actors"][0]["dnd5e"] = json!({"build":{"secret":true},"level":1});
        let projected = snapshot(&w, &player(), 0);
        assert!(
            projected["world"]["actors"][0]["dnd5e"]
                .get("build")
                .is_none()
        );
        assert_eq!(projected["world"]["actors"][0]["dnd5e"]["level"], 1);
    }
    #[test]
    fn combat_rounds() {
        let mut w = demo();
        let gm = User {
            name: "MJ".into(),
            role: "gm".into(),
        };
        apply(&mut w, &gm, &json!({"kind":"combat_add","id":"hero"})).unwrap();
        apply(&mut w, &gm, &json!({"kind":"combat_next"})).unwrap();
        assert_eq!(w["combat"]["round"], 2);
        assert!(validate_world(&w).is_ok());
        assert!(validate_world(&json!({})).is_err());
    }
}
