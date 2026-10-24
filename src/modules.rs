//! Module registry for the public edition.
use super::*;
pub fn manifest(w: &Value, _u: &User) -> Value {
    json!([builder::manifest(w)])
}
pub async fn list_modules(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    let i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    Ok(Json(manifest(&i.world, &u)))
}
pub async fn list_packs(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    let i = app.inner.lock().await;
    user(&headers, &i)?;
    Ok(Json(json!({"packs":[],"enabled":false})))
}
pub fn apply(w: &mut Value, u: &User, v: &Value) -> Result<bool, (StatusCode, String)> {
    if v["kind"] != "module_toggle" {
        return Ok(false);
    }
    if u.role != "gm" {
        return Err(forbidden());
    }
    if v["id"] != "dnd5e-character-builder" {
        return Err(bad("Module inconnu"));
    }
    let on = v["enabled"].as_bool().ok_or(bad("État du module requis"))?;
    if w.get("modules").is_none() {
        w["modules"] = json!({});
    }
    w["modules"]["dnd5e-character-builder"] = json!({"enabled":on});
    Ok(true)
}
