//! Native Rust conversion of the Character Builder workflow, using the French SRD 5.2.1.
use super::*;
use axum::extract::Path;
use std::sync::OnceLock;
const ID: &str = "dnd5e-character-builder";
const ABILITIES: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];
pub fn catalog() -> &'static Value {
    static C: OnceLock<Value> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../modules/dnd5e-character-builder/catalog.fr.json"
        ))
        .expect("Catalogue 5e français invalide")
    })
}
pub fn enabled(w: &Value) -> bool {
    w["modules"][ID]["enabled"] == true
}
pub fn manifest(w: &Value) -> Value {
    let c = catalog();
    json!({"id":ID,"title":c["title"],"description":"Création guidée, progression, multiclassage et fiche 5e en français.","version":c["version"],"revision":c["revision"],"source":c["source"],"authors":c["authors"],"license":"MIT / SRD CC-BY-4.0","enabled":enabled(w),"pack_count":12,"document_count":339,"adapter":"Assistant natif Rust en français, règles et contenu du SRD 5.2.1 ; extensions Foundry non exécutées.","requires":[]})
}
fn require(w: &Value) -> Result<(), (StatusCode, String)> {
    if enabled(w) {
        Ok(())
    } else {
        Err((
            StatusCode::CONFLICT,
            "Activez le Créateur de personnage 5e dans Gérer les modules".into(),
        ))
    }
}
pub fn settings(w: &Value) -> Value {
    let mut c = json!({"allow_creation":true,"allow_manual":false,"allow_multiclass":false,"bonus_gold":0,"require_level_grant":true});
    if let Some(s) = w["builder_settings"].as_object() {
        for k in [
            "allow_creation",
            "allow_manual",
            "allow_multiclass",
            "bonus_gold",
            "require_level_grant",
        ] {
            if let Some(v) = s.get(k) {
                c[k] = v.clone()
            }
        }
    }
    c
}
pub async fn get_catalog(State(app): State<App>, headers: HeaderMap) -> ApiResult {
    let i = app.inner.lock().await;
    user(&headers, &i)?;
    require(&i.world)?;
    Ok(Json(
        json!({"catalog":catalog(),"settings":settings(&i.world)}),
    ))
}
pub async fn reference(
    State(app): State<App>,
    headers: HeaderMap,
    Path(page): Path<String>,
) -> ApiResult {
    let i = app.inner.lock().await;
    user(&headers, &i)?;
    require(&i.world)?;
    static P: OnceLock<Value> = OnceLock::new();
    let p = P.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../modules/dnd5e-character-builder/reference-pages.fr.json"
        ))
        .unwrap()
    });
    let t = p.get(&page).ok_or(bad("Page SRD inconnue"))?;
    Ok(Json(json!({"page":page,"text":t})))
}
fn lookup(kind: &str, id: &str) -> Option<&'static Value> {
    catalog()[kind].as_array()?.iter().find(|x| x["id"] == id)
}
fn list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
fn modifier(n: i64) -> i64 {
    (n - 10).div_euclid(2)
}
fn integer(v: &Value, default: i64) -> i64 {
    v.as_i64()
        .or_else(|| v.as_f64().map(|n| n as i64))
        .unwrap_or(default)
}
fn draft<'a>(w: &'a Value, u: &User, id: &Value) -> Result<&'a Value, (StatusCode, String)> {
    let d = w["builder_drafts"]
        .as_array()
        .and_then(|a| a.iter().find(|d| d["id"] == *id))
        .ok_or(bad("Brouillon introuvable"))?;
    if u.role != "gm" && d["owner"] != u.name {
        return Err(forbidden());
    }
    Ok(d)
}
fn drafts(w: &mut Value) -> &mut Vec<Value> {
    if w.get("builder_drafts").is_none() {
        w["builder_drafts"] = json!([]);
    }
    w["builder_drafts"].as_array_mut().unwrap()
}
fn progress(class: &Value, level: i64) -> Value {
    class["progression"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p["level"] == level))
        .cloned()
        .unwrap_or(json!({"level":level,"cantrips":0,"prepared":0,"slots":[],"features":""}))
}
fn selected_classes(w: &Value, d: &Value) -> Result<Vec<Value>, String> {
    let data = &d["data"];
    let class =
        lookup("classes", data["class"].as_str().unwrap_or("")).ok_or("Choisissez une classe.")?;
    if d["mode"] != "levelup" {
        return Ok(vec![
            json!({"id":class["id"],"name":class["name"],"level":1,"hd":class["hd"]}),
        ]);
    }
    let a = w["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == d["actor_id"])
        .ok_or("Personnage introuvable.")?;
    let mut classes = a["dnd5e"]["classes"]
        .as_array()
        .ok_or("Fiche 5e invalide.")?
        .clone();
    if let Some(c) = classes.iter_mut().find(|c| c["id"] == class["id"]) {
        c["level"] = json!(c["level"].as_i64().unwrap_or(1) + 1);
    } else {
        classes.push(json!({"id":class["id"],"name":class["name"],"level":1,"hd":class["hd"]}));
    }
    Ok(classes)
}
fn assemble(w: &Value, d: &Value) -> Result<Value, Vec<String>> {
    let data = &d["data"];
    let mut errors = vec![];
    let name = data["name"].as_str().unwrap_or("").trim();
    if name.is_empty() || name.len() > 100 {
        errors.push("Indiquez un nom de personnage (100 caractères maximum).".into());
    }
    let class = lookup("classes", data["class"].as_str().unwrap_or(""));
    let species = lookup("species", data["species"].as_str().unwrap_or(""));
    let bg = lookup("backgrounds", data["background"].as_str().unwrap_or(""));
    if class.is_none() {
        errors.push("Choisissez une classe.".into());
    }
    if species.is_none() {
        errors.push("Choisissez une espèce.".into());
    }
    if bg.is_none() {
        errors.push("Choisissez un historique.".into());
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let class = class.unwrap();
    let species = species.unwrap();
    let bg = bg.unwrap();
    let levelup = d["mode"] == "levelup";
    let old = w["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == d["actor_id"]);
    if levelup && old.is_none() {
        return Err(vec!["Le personnage source a disparu.".into()]);
    }
    let classes = selected_classes(w, d).map_err(|e| vec![e])?;
    let level: i64 = classes
        .iter()
        .map(|c| c["level"].as_i64().unwrap_or(1))
        .sum();
    if level > 20 {
        return Err(vec!["Le niveau total ne peut pas dépasser 20.".into()]);
    }
    let mut base = vec![];
    let mut scores = json!({});
    let mut boosts = vec![];
    for key in ABILITIES {
        let n = data["scores"][key].as_i64().unwrap_or(0);
        if !(3..=18).contains(&n) {
            errors.push(format!("Valeur de {key} : de 3 à 18 avant bonus."));
        }
        base.push(n);
        let b = data["boosts"][key].as_i64().unwrap_or(0);
        if !(0..=2).contains(&b) || (b > 0 && !list(&bg["abilities"]).contains(&key.to_string())) {
            errors.push(
                "Les bonus d’historique doivent respecter ses trois caractéristiques.".into(),
            );
        }
        boosts.push(b);
        scores[key] = json!(n + b);
    }
    let mut boost_pattern = boosts.clone();
    boost_pattern.retain(|n| *n > 0);
    boost_pattern.sort();
    if boost_pattern != [1, 2] && boost_pattern != [1, 1, 1] {
        errors.push("Répartissez les bonus d’historique : +2/+1 ou +1/+1/+1.".into());
    }
    if !levelup {
        match data["method"].as_str().unwrap_or("standard") {
            "standard" => {
                let mut sorted = base.clone();
                sorted.sort();
                if sorted != [8, 10, 12, 13, 14, 15] {
                    errors.push("La série standard doit utiliser une fois chacune les valeurs 15, 14, 13, 12, 10 et 8.".into());
                }
            }
            "pointbuy" => {
                let costs = [0, 1, 2, 3, 4, 5, 7, 9];
                let mut spent = 0;
                for n in &base {
                    if !(8..=15).contains(n) {
                        errors.push("L’achat de points autorise des valeurs de 8 à 15.".into())
                    } else {
                        spent += costs[(*n - 8) as usize];
                    }
                }
                if spent > 27 {
                    errors.push("Le budget de caractéristiques est limité à 27 points.".into());
                }
            }
            "rolled" => {
                let mut rolled: Vec<i64> = d["rolled_scores"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default();
                rolled.sort();
                let mut s = base.clone();
                s.sort();
                if rolled.len() != 6 || rolled != s {
                    errors.push(
                        "Affectez les six résultats tirés par le serveur, sans en réutiliser un."
                            .into(),
                    );
                }
            }
            "manual" => {
                if settings(w)["allow_manual"] != true && d["owner_role"] != "gm" {
                    errors.push("La saisie manuelle est désactivée par le MJ.".into());
                }
            }
            _ => errors.push("Méthode de caractéristiques inconnue.".into()),
        }
    } else {
        if data["scores"] != old.unwrap()["dnd5e"]["build"]["scores"]
            || data["boosts"] != old.unwrap()["dnd5e"]["build"]["boosts"]
            || data["species"] != old.unwrap()["dnd5e"]["build"]["species"]
            || data["background"] != old.unwrap()["dnd5e"]["build"]["background"]
        {
            errors.push("Une progression ne peut pas changer les caractéristiques d’origine, l’espèce ou l’historique.".into());
        }
        if classes.len() > old.unwrap()["dnd5e"]["classes"].as_array().unwrap().len()
            && settings(w)["allow_multiclass"] != true
        {
            errors.push("Le multiclassage est désactivé par le MJ.".into());
        }
    }
    // Carry historical improvements, then apply a new ASI only on eligible class levels.
    let mut improvements = old
        .map(|a| a["dnd5e"]["improvements"].clone())
        .filter(Value::is_object)
        .unwrap_or(json!({}));
    let new_class_level = classes.iter().find(|c| c["id"] == class["id"]).unwrap()["level"]
        .as_i64()
        .unwrap_or(1);
    let cid = class["id"].as_str().unwrap();
    let asi = levelup
        && ([4, 8, 12, 16].contains(&new_class_level)
            || (cid == "guerrier" && [6, 14].contains(&new_class_level))
            || (cid == "roublard" && new_class_level == 10));
    if asi {
        let mut total = 0;
        for k in ABILITIES {
            let n = data["asi"][k].as_i64().unwrap_or(0);
            if !(0..=2).contains(&n) {
                errors.push("L’amélioration accorde deux points au total.".into());
            }
            total += n;
            improvements[k] = json!(improvements[k].as_i64().unwrap_or(0) + n);
        }
        if total != 2 {
            errors.push("Répartissez les deux points d’amélioration de caractéristique.".into());
        }
    }
    for k in ABILITIES {
        let n = scores[k].as_i64().unwrap_or(10) + improvements[k].as_i64().unwrap_or(0);
        if n > 20 {
            errors.push("Une caractéristique ne peut pas dépasser 20.".into());
        }
        scores[k] = json!(n);
    }
    if classes.len() > 1 {
        for c in &classes {
            let cl = lookup("classes", c["id"].as_str().unwrap()).unwrap();
            for requirement in list(&cl["prerequisites"]) {
                if !requirement
                    .split('|')
                    .any(|a| scores[a].as_i64().unwrap_or(0) >= 13)
                {
                    errors.push(format!(
                        "Multiclassage : {} nécessite au moins 13 en {}.",
                        cl["name"].as_str().unwrap(),
                        requirement
                    ));
                }
            }
        }
    }
    if !list(&species["lineages"]).is_empty()
        && !list(&species["lineages"]).contains(&data["lineage"].as_str().unwrap_or("").to_string())
    {
        errors.push("Choisissez un lignage ou une ascendance pour cette espèce.".into());
    }
    let primary_id = classes[0]["id"].as_str().unwrap();
    let primary = lookup("classes", primary_id).unwrap();
    let selected_skills = list(&data["skills"]);
    let background_skills = list(&bg["skills"]);
    let mut proficiencies = background_skills.clone();
    if selected_skills.len() != primary["skill_count"].as_u64().unwrap() as usize {
        errors.push(format!(
            "Choisissez {} compétences de classe.",
            primary["skill_count"]
        ));
    }
    for skill in &selected_skills {
        if !list(&primary["skills"]).contains(skill) || proficiencies.contains(skill) {
            errors.push("Chaque compétence de classe doit être autorisée et différente des maîtrises déjà obtenues.".into());
        } else {
            proficiencies.push(skill.clone());
        }
    }
    let human = species["id"] == "humain";
    let feat = if human {
        data["human_feat"].as_str().unwrap_or("")
    } else {
        ""
    };
    if human && !list(&catalog()["feats"]).contains(&feat.to_string()) {
        errors.push("Choisissez le don d’origines supplémentaire de l’humain.".into());
    }
    let extra = list(&data["extra_skills"]);
    let extra_count = if human {
        1 + if feat == "Doué" { 3 } else { 0 }
    } else {
        0
    };
    if extra.len() != extra_count {
        errors.push(format!(
            "Choisissez {extra_count} compétences supplémentaires d’espèce/don."
        ));
    }
    for s in extra {
        if lookup("skills", &s).is_none() || proficiencies.contains(&s) {
            errors
                .push("Les compétences supplémentaires doivent être distinctes et valides.".into());
        } else {
            proficiencies.push(s)
        }
    }
    // Validate spell selections per class against the French SRD and source progression tables.
    let mut known: HashMap<String, Value> = HashMap::new();
    let mut spellcasting = vec![];
    for c in &classes {
        let cl = lookup("classes", c["id"].as_str().unwrap()).unwrap();
        if cl["casting"].is_null() {
            continue;
        }
        let lvl = c["level"].as_i64().unwrap();
        let p = progress(cl, lvl);
        let slots = p["slots"].as_array().cloned().unwrap_or_default();
        let maxspell = slots
            .iter()
            .rposition(|n| n.as_i64().unwrap_or(0) > 0)
            .map(|n| n + 1)
            .unwrap_or(0) as i64;
        let choices = &data["spell_choices"][c["id"].as_str().unwrap()];
        let cans = list(&choices["cantrips"]);
        let prepared = list(&choices["prepared"]);
        let book = list(&choices["book"]);
        if cl["id"] != "magicien" && !book.is_empty() {
            errors.push("Seul le magicien utilise un grimoire dans cet assistant.".into());
        }
        let nc = p["cantrips"].as_u64().unwrap_or(0) as usize;
        let np = p["prepared"].as_u64().unwrap_or(0) as usize;
        if cans.len() != nc || prepared.len() != np {
            errors.push(format!(
                "{} : choisissez {nc} sorts mineurs et {np} sorts préparés.",
                cl["name"].as_str().unwrap()
            ));
        }
        if cl["id"] == "magicien" {
            let required = 6 + 2 * (lvl - 1) as usize;
            if book.len() != required {
                errors.push(format!(
                    "Le grimoire du magicien doit contenir {required} sorts."
                ));
            }
            if prepared.iter().any(|s| !book.contains(s)) {
                errors
                    .push("Les sorts préparés du magicien doivent appartenir au grimoire.".into());
            }
            if let Some(old) = old {
                let previous = list(&old["dnd5e"]["build"]["spell_choices"]["magicien"]["book"]);
                if previous.iter().any(|s| !book.contains(s)) {
                    errors
                        .push("Les sorts déjà inscrits au grimoire doivent être conservés.".into());
                }
            }
        }
        for (ids, minor, role) in [
            (&cans, true, "mineur"),
            (&prepared, false, "préparé"),
            (&book, false, "grimoire"),
        ] {
            let mut seen = std::collections::HashSet::new();
            for sid in ids {
                if !seen.insert(sid) {
                    errors.push(
                        "Un même sort ne peut pas être choisi deux fois dans une liste.".into(),
                    );
                }
                let Some(spell) = lookup("spells", sid) else {
                    errors.push("Sort inconnu.".into());
                    continue;
                };
                let sl = spell["level"].as_i64().unwrap();
                if !list(&spell["classes"]).contains(&cl["name"].as_str().unwrap().to_string())
                    || (minor && sl != 0)
                    || (!minor && (sl == 0 || sl > maxspell))
                {
                    errors.push(format!(
                        "{} ne peut pas choisir ce sort à ce niveau : {}.",
                        cl["name"].as_str().unwrap(),
                        spell["name"].as_str().unwrap()
                    ));
                }
                let e=known.entry(sid.clone()).or_insert_with(||json!({"id":sid,"name":spell["name"],"level":spell["level"],"school":spell["school"],"description":spell["description"],"page":spell["page"],"prepared":false,"owners":[]}));
                if minor || role == "préparé" {
                    e["prepared"] = json!(true);
                }
                e["owners"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"class":cl["id"],"role":role}));
            }
        }
        let ability = cl["casting"].as_str().unwrap();
        spellcasting.push(json!({"class":cl["id"],"name":cl["name"],"ability":ability,"cantrips":nc,"prepared":np,"slots":slots,"pact":cl["id"]=="occultiste","dc":8+2+(level-1)/4+modifier(scores[ability].as_i64().unwrap()),"attack":2+(level-1)/4+modifier(scores[ability].as_i64().unwrap())}));
    }
    let mut magic_sources = vec![];
    if let Some(m) = bg["magic"].as_str() {
        magic_sources.push(m.to_string());
    }
    if human && feat.starts_with("Initié à la magie") {
        let target = if feat.contains("Clerc") {
            "clerc"
        } else if feat.contains("Druide") {
            "druide"
        } else {
            "magicien"
        };
        if magic_sources.iter().any(|m| m == target) {
            errors.push(
                "L’Initié à la magie supplémentaire doit utiliser une autre liste de sorts.".into(),
            );
        } else {
            magic_sources.push(target.into());
        }
    }
    for source in magic_sources {
        let cl = lookup("classes", &source).unwrap();
        let m = &data["origin_magic"][&source];
        let cans = list(&m["cantrips"]);
        let ps = list(&m["prepared"]);
        if cans.iter().collect::<std::collections::HashSet<_>>().len() != cans.len() {
            errors.push("Les sorts mineurs d’origines doivent être différents.".into());
        }
        if cans.len() != 2
            || ps.len() != 1
            || !["int", "wis", "cha"].contains(&m["ability"].as_str().unwrap_or(""))
        {
            errors.push(format!("Initié à la magie ({}) : choisissez deux sorts mineurs, un sort du 1er niveau et une caractéristique d’incantation.",cl["name"].as_str().unwrap()));
        }
        for sid in cans.iter().chain(ps.iter()) {
            let Some(s) = lookup("spells", sid) else {
                errors.push("Sort d’origines inconnu.".into());
                continue;
            };
            if !list(&s["classes"]).contains(&cl["name"].as_str().unwrap().to_string())
                || s["level"].as_i64().unwrap() > 1
                || (cans.contains(sid) && s["level"] != 0)
                || (ps.contains(sid) && s["level"] != 1)
            {
                errors.push("Sort d’origines non autorisé.".into());
            }
            let e = known.entry(sid.clone()).or_insert_with(|| {
                let mut s = s.clone();
                s["owners"] = json!([]);
                s
            });
            e["prepared"] = json!(true);
            e["owners"]
                .as_array_mut()
                .unwrap()
                .push(json!({"origin":source,"ability":m["ability"],"free_cast":s["level"]==1}));
        }
    }
    // Starting money and shop use copper integers; client prices are never trusted.
    let mut inventory = vec![];
    let mut money = 0i64;
    if !levelup {
        if !["pack", "gold"].contains(&data["class_equipment"].as_str().unwrap_or("")) {
            errors.push("Choisissez l’équipement ou l’or de départ de classe.".into());
        }
        if !["pack", "gold"].contains(&data["background_equipment"].as_str().unwrap_or("")) {
            errors.push("Choisissez l’équipement ou l’or de l’historique.".into());
        }
        if data["class_equipment"] == "gold" {
            money += class["starting_gold"].as_i64().unwrap() * 100;
        } else {
            money += class["pack_gold"].as_i64().unwrap() * 100;
            inventory.push(json!({"name":"Équipement de classe","description":class["equipment"],"quantity":1}));
        }
        if data["background_equipment"] == "gold" {
            money += 5000;
        } else {
            money += bg["gold"].as_i64().unwrap() * 100;
            inventory.push(json!({"name":"Équipement d’historique","description":bg["equipment"],"quantity":1}));
        }
        money += settings(w)["bonus_gold"].as_i64().unwrap_or(0) * 100;
        if let Some(cart) = data["cart"].as_object() {
            for (item_id, qty) in cart {
                let n = qty.as_i64().unwrap_or(-1);
                if !(0..=99).contains(&n) {
                    errors.push("Quantité d’équipement invalide (0 à 99).".into());
                    continue;
                }
                let Some(item) = lookup("equipment", item_id) else {
                    errors.push("Objet de boutique inconnu.".into());
                    continue;
                };
                money -= n * item["cost"].as_i64().unwrap();
                if n > 0 {
                    let mut item = item.clone();
                    item["quantity"] = json!(n);
                    inventory.push(item);
                }
            }
        }
        if money < 0 {
            errors.push("Les achats dépassent le budget disponible.".into());
        }
    } else {
        inventory = old.unwrap()["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        money = old.unwrap()["dnd5e"]["currency_cp"].as_i64().unwrap_or(0);
    }
    if !errors.is_empty() {
        errors.sort();
        errors.dedup();
        return Err(errors);
    }
    let con = modifier(scores["con"].as_i64().unwrap());
    let dex = modifier(scores["dex"].as_i64().unwrap());
    let wis = modifier(scores["wis"].as_i64().unwrap());
    let prof = 2 + (level - 1) / 4;
    let mut hp_dice = old
        .map(|a| {
            a["dnd5e"]["hp_dice"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if levelup {
        let hd = class["hd"].as_i64().unwrap();
        let n = if data["hp_method"] == "rolled" {
            if d["hp_roll_class"] != class["id"] {
                return Err(vec![
                    "Le dé de vie tiré est lié à une autre classe. Choisissez le gain fixe.".into(),
                ]);
            }
            d["hp_roll"].as_i64().ok_or(vec![
                "Tirez le dé de vie sur le serveur ou choisissez le gain fixe.".into(),
            ])?
        } else {
            hd / 2 + 1
        };
        hp_dice.push(json!({"class":cid,"die":hd,"value":n}));
    } else {
        hp_dice.push(json!({"class":cid,"die":class["hd"],"value":class["hd"]}));
    }
    let hp: i64 = hp_dice
        .iter()
        .map(|d| (d["value"].as_i64().unwrap() + con).max(1))
        .sum::<i64>()
        + if species["id"] == "nain" { level } else { 0 };
    let mut ac = 10 + dex;
    if primary_id == "barbare" {
        ac = ac.max(10 + dex + con);
    }
    if primary_id == "moine" {
        ac = ac.max(10 + dex + wis);
    }
    // The starting pack's armor is used for its initial AC; the shop can equip one armor and shield.
    if data["class_equipment"] == "pack" {
        ac = match primary_id {
            "guerrier" | "paladin" => 16,
            "clerc" => 13 + dex.min(2) + 2,
            "druide" => 11 + dex + 2,
            "barde" | "occultiste" | "roublard" => 11 + dex,
            "rodeur" => 12 + dex,
            _ => ac,
        };
        if primary_id == "paladin" {
            ac += 2;
        }
    }
    let armor_id = data["equipped_armor"].as_str().unwrap_or("");
    if !armor_id.is_empty() {
        if let Some(item) = lookup("equipment", armor_id) {
            if inventory.iter().any(|i| i["id"] == armor_id) {
                ac = match item["formula"].as_str().unwrap_or("") {
                    "11+dex" => 11 + dex,
                    "12+dex" => 12 + dex,
                    "13+dex2" => 13 + dex.min(2),
                    "16" => 16,
                    _ => ac,
                };
            }
        }
    }
    if data["shield"] == true && inventory.iter().any(|i| i["id"] == "bouclier") {
        ac += 2;
    }
    let mut features = vec![];
    for c in &classes {
        let cl = lookup("classes", c["id"].as_str().unwrap()).unwrap();
        for p in cl["progression"].as_array().unwrap() {
            if p["level"].as_i64().unwrap_or(99) <= c["level"].as_i64().unwrap() {
                features.push(json!({"class":cl["name"],"level":p["level"],"name":p["features"],"page":cl["page"]}));
            }
        }
    }
    let lineage = data["lineage"].as_str().unwrap_or("");
    let mut racial = vec![];
    match lineage {
        "Drow" => {
            racial.push("Lumières dansantes");
            if level >= 3 {
                racial.push("Lueurs féeriques");
            }
            if level >= 5 {
                racial.push("Ténèbres");
            }
        }
        "Elfe sylvestre" => {
            racial.push("Druidisme");
            if level >= 3 {
                racial.push("Grande foulée");
            }
            if level >= 5 {
                racial.push("Passage sans trace");
            }
        }
        "Haut-elfe" => {
            racial.push("Prestidigitation");
            if level >= 3 {
                racial.push("Détection de la magie");
            }
            if level >= 5 {
                racial.push("Foulée brumeuse");
            }
        }
        "Gnome des forêts" => {
            racial.extend(["Illusion mineure", "Communication avec les animaux"])
        }
        "Gnome des roches" => racial.extend(["Prestidigitation", "Réparation"]),
        "Abyssal" => {
            racial.extend(["Thaumaturgie", "Bouffée de poison"]);
            if level >= 3 {
                racial.push("Rayon empoisonné");
            }
            if level >= 5 {
                racial.push("Immobilisation de personne");
            }
        }
        "Chtonien" => {
            racial.extend(["Thaumaturgie", "Contact glacial"]);
            if level >= 3 {
                racial.push("Simulacre de vie");
            }
            if level >= 5 {
                racial.push("Rayon affaiblissant");
            }
        }
        "Infernal" => {
            racial.extend(["Thaumaturgie", "Trait de feu"]);
            if level >= 3 {
                racial.push("Représailles infernales");
            }
            if level >= 5 {
                racial.push("Ténèbres");
            }
        }
        _ => {}
    }
    for name in racial {
        if let Some(spell) = catalog()["spells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"].as_str().unwrap_or("").to_lowercase() == name.to_lowercase())
        {
            let sid = spell["id"].as_str().unwrap().to_string();
            let e = known.entry(sid).or_insert_with(|| {
                let mut s = spell.clone();
                s["owners"] = json!([]);
                s
            });
            e["prepared"] = json!(true);
            e["owners"].as_array_mut().unwrap().push(json!({"species":species["id"],"lineage":lineage,"ability":data["racial_casting"].as_str().filter(|a|["int","wis","cha"].contains(a)).unwrap_or("int")}));
        }
    }
    let nonpact: Vec<_> = spellcasting.iter().filter(|s| s["pact"] != true).collect();
    let shared_slots = if nonpact.len() > 1 {
        let caster_level: i64 = classes
            .iter()
            .map(|c| {
                let id = c["id"].as_str().unwrap();
                let n = c["level"].as_i64().unwrap();
                if ["paladin", "rodeur"].contains(&id) {
                    (n + 1) / 2
                } else if ["barde", "clerc", "druide", "ensorceleur", "magicien"].contains(&id) {
                    n
                } else {
                    0
                }
            })
            .sum();
        progress(lookup("classes", "clerc").unwrap(), caster_level.min(20))["slots"].clone()
    } else {
        nonpact
            .first()
            .map(|s| s["slots"].clone())
            .unwrap_or(json!([]))
    };
    let mut spell_vec: Vec<Value> = known.into_values().collect();
    spell_vec.sort_by_key(|s| format!("{}{}", s["level"], s["name"]));
    let actor_id = if levelup {
        d["actor_id"].clone()
    } else {
        json!(id())
    };
    let owner = if levelup {
        old.unwrap()["owner"].clone()
    } else {
        d["owner"].clone()
    };
    let current_hp = if levelup {
        (integer(&old.unwrap()["hp"], 0) + (hp - integer(&old.unwrap()["max_hp"], 0))).clamp(0, hp)
    } else {
        hp
    };
    Ok(
        json!({"id":actor_id,"name":name,"owner":owner,"hp":current_hp,"max_hp":hp,"armor":ac,"initiative":dex+if bg["feat"]=="Vigilant"||feat=="Vigilant"{prof}else{0},"notes":data["notes"].as_str().unwrap_or(""),"items":inventory,"system_id":"dnd5e","dnd5e":{"rules":"SRD 5.2.1 / 2024","language":"fr","classes":classes,"level":level,"species":species["name"],"lineage":data["lineage"],"background":bg["name"],"background_feat":bg["feat"],"human_feat":feat,"abilities":scores,"improvements":improvements,"proficiency":prof,"skills":proficiencies,"saves":primary["saves"],"speed":if data["lineage"]=="Elfe sylvestre"{json!(10.5)}else{species["speed"].clone()},"traits":species["traits"],"spells":spell_vec,"spellcasting":spellcasting,"shared_slots":shared_slots,"spent_slots":old.map(|a|a["dnd5e"]["spent_slots"].clone()).filter(Value::is_object).unwrap_or(json!({})),"currency_cp":money,"hp_dice":hp_dice,"hit_dice_spent":old.map(|a|a["dnd5e"]["hit_dice_spent"].as_i64().unwrap_or(0)).unwrap_or(0),"features":features,"build":data,"created_by":"Rusty VTT / Character Builder FR","source_revision":catalog()["revision"]}}),
    )
}
pub async fn preview(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    let i = app.inner.lock().await;
    let u = user(&headers, &i)?;
    require(&i.world)?;
    let d = draft(&i.world, &u, &json!(id))?;
    Ok(Json(match assemble(&i.world, d) {
        Ok(a) => json!({"valid":true,"errors":[],"actor":a}),
        Err(errors) => json!({"valid":false,"errors":errors}),
    }))
}
pub fn apply(w: &mut Value, u: &User, v: &Value) -> Result<bool, (StatusCode, String)> {
    let kind = v["kind"].as_str().unwrap_or("");
    if !kind.starts_with("builder_") {
        return Ok(false);
    }
    require(w)?;
    match kind {
        "builder_settings" => {
            if u.role != "gm" {
                return Err(forbidden());
            }
            let mut s = settings(w);
            for k in [
                "allow_creation",
                "allow_manual",
                "allow_multiclass",
                "require_level_grant",
            ] {
                if let Some(value) = v.get(k) {
                    if !value.is_boolean() {
                        return Err(bad("Paramètre booléen requis"));
                    }
                    s[k] = value.clone();
                }
            }
            if v.get("bonus_gold").is_some() {
                s["bonus_gold"] = json!(num(v, "bonus_gold", 0., 10000.)? as i64);
            }
            w["builder_settings"] = s;
        }
        "builder_draft_create" => {
            if u.role != "gm" && settings(w)["allow_creation"] != true {
                return Err(forbidden());
            }
            if w["builder_drafts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|d| d["owner"] == u.name && d["status"] != "committed")
                        .count()
                })
                .unwrap_or(0)
                >= 20
            {
                return Err(bad("Maximum : 20 brouillons actifs par joueur"));
            }
            drafts(w).push(json!({"id":id(),"owner":u.name,"owner_role":u.role,"mode":"create","version":0,"status":"draft","stage":0,"data":{"name":"","class":"guerrier","species":"humain","background":"soldat","method":"standard","scores":{"str":15,"dex":14,"con":13,"int":12,"wis":10,"cha":8},"boosts":{"str":2,"dex":0,"con":1,"int":0,"wis":0,"cha":0},"skills":["acrobaties","dressage"],"extra_skills":["perception","arcanes","histoire","intuition"],"human_feat":"Doué","class_equipment":"pack","background_equipment":"pack","cart":{},"spell_choices":{},"origin_magic":{}}}));
        }
        "builder_level_grant" => {
            if u.role != "gm" {
                return Err(forbidden());
            }
            let a = find(collection(w, "actors")?, &v["actor_id"])?;
            if a["system_id"] != "dnd5e" {
                return Err(bad("Personnage 5e requis"));
            }
            a["dnd5e"]["level_granted"] = json!(v["granted"].as_bool().unwrap_or(true));
        }
        "builder_level_start" => {
            let a = find(collection(w, "actors")?, &v["actor_id"])?.clone();
            if !owned(u, &a) || a["system_id"] != "dnd5e" {
                return Err(forbidden());
            }
            if settings(w)["require_level_grant"] == true
                && u.role != "gm"
                && a["dnd5e"]["level_granted"] != true
            {
                return Err(bad("Le MJ doit autoriser le prochain niveau"));
            }
            if a["dnd5e"]["level"].as_i64().unwrap_or(20) >= 20 {
                return Err(bad("Niveau maximum atteint"));
            }
            if drafts(w)
                .iter()
                .any(|d| d["actor_id"] == a["id"] && d["status"] == "draft")
            {
                return Err(bad("Un brouillon de progression existe déjà"));
            }
            let mut data = a["dnd5e"]["build"].clone();
            data["notes"] = a["notes"].clone();
            data["asi"] = json!({});
            data["hp_method"] = json!("fixed");
            drafts(w).push(json!({"id":id(),"owner":u.name,"owner_role":u.role,"mode":"levelup","actor_id":a["id"],"base_actor":a,"version":0,"status":"draft","stage":2,"data":data}));
        }
        "builder_draft_save" => {
            let d = draft(w, u, &v["id"])?.clone();
            if d["status"] != "draft" {
                return Err(bad("Brouillon déjà finalisé"));
            }
            if d["version"] != v["version"] {
                return Err((
                    StatusCode::CONFLICT,
                    "Ce brouillon a changé dans une autre fenêtre. Rechargez-le.".into(),
                ));
            }
            if !v["data"].is_object() || serde_json::to_vec(&v["data"]).unwrap().len() > 200000 {
                return Err(bad("Brouillon trop volumineux ou invalide"));
            }
            let target = find(drafts(w), &v["id"])?;
            target["data"] = v["data"].clone();
            target["stage"] = json!(v["stage"].as_u64().unwrap_or(0).min(7));
            target["version"] = json!(d["version"].as_u64().unwrap_or(0) + 1);
        }
        "builder_roll_scores" => {
            let d = draft(w, u, &v["id"])?.clone();
            if d["status"] != "draft" || d["mode"] == "levelup" {
                return Err(bad("Tirage indisponible"));
            }
            if d["rolled_scores"].is_array() {
                return Ok(true);
            }
            let mut values = vec![];
            let mut details = vec![];
            for _ in 0..6 {
                let mut dice: Vec<i64> = (0..4).map(|_| rand::rng().random_range(1..=6)).collect();
                dice.sort();
                values.push(dice[1..].iter().sum::<i64>());
                details.push(dice);
            }
            let target = find(drafts(w), &v["id"])?;
            target["rolled_scores"] = json!(values);
            target["roll_details"] = json!(details);
        }
        "builder_roll_hp" => {
            let d = draft(w, u, &v["id"])?.clone();
            if d["mode"] != "levelup" || d["status"] != "draft" {
                return Err(bad("Progression requise"));
            }
            if d["hp_roll"].is_number() {
                return Ok(true);
            }
            let cl = lookup("classes", d["data"]["class"].as_str().unwrap_or(""))
                .ok_or(bad("Classe inconnue"))?;
            let target = find(drafts(w), &v["id"])?;
            target["hp_roll"] = json!(rand::rng().random_range(1..=cl["hd"].as_i64().unwrap()));
            target["hp_roll_class"] = cl["id"].clone();
        }
        "builder_commit" => {
            let d = draft(w, u, &v["id"])?.clone();
            if d["status"] == "committed" {
                return Ok(true);
            }
            if d["version"] != v["version"] {
                return Err(bad("Le brouillon a changé : relisez l’aperçu"));
            }
            if d["mode"] == "levelup" {
                let live = w["actors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|a| a["id"] == d["actor_id"])
                    .ok_or(bad("Personnage introuvable"))?;
                if !owned(u, live) {
                    return Err(forbidden());
                }
                if live != &d["base_actor"] {
                    return Err((StatusCode::CONFLICT,"La fiche a changé depuis le début de la progression. Reprenez un nouveau brouillon.".into()));
                }
            }
            let a = assemble(w, &d).map_err(|e| bad(&e.join("\n")))?;
            let actor_id = a["id"].clone();
            if d["mode"] == "levelup" {
                let old = find(collection(w, "actors")?, &actor_id)?;
                *old = a;
            } else {
                collection(w, "actors")?.push(a);
            }
            let target = find(drafts(w), &v["id"])?;
            target["status"] = json!("committed");
            target["created_actor"] = actor_id;
        }
        "builder_draft_discard" => {
            let _ = draft(w, u, &v["id"])?;
            drafts(w).retain(|d| d["id"] != v["id"]);
        }
        "builder_rest" => {
            let a = find(collection(w, "actors")?, &v["actor_id"])?;
            if !owned(u, a) || a["system_id"] != "dnd5e" {
                return Err(forbidden());
            }
            let long = v["rest"] == "long";
            if !long && v["rest"] != "short" {
                return Err(bad("Repos inconnu"));
            }
            if long {
                a["hp"] = a["max_hp"].clone();
                a["dnd5e"]["hit_dice_spent"] = json!(0);
                a["dnd5e"]["spent_slots"] = json!({});
            } else {
                let n = v["hit_dice"].as_i64().unwrap_or(0);
                let total = a["dnd5e"]["level"].as_i64().unwrap_or(1);
                let used = a["dnd5e"]["hit_dice_spent"].as_i64().unwrap_or(0);
                if n < 0 || n > total - used {
                    return Err(bad("Pas assez de dés de vie disponibles"));
                }
                let con = modifier(a["dnd5e"]["abilities"]["con"].as_i64().unwrap_or(10));
                let dice = a["dnd5e"]["hp_dice"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let heal: i64 = dice
                    .iter()
                    .skip(used as usize)
                    .take(n as usize)
                    .map(|d| {
                        (rand::rng().random_range(1..=d["die"].as_i64().unwrap_or(8)) + con).max(0)
                    })
                    .sum();
                a["hp"] = json!((integer(&a["hp"], 0) + heal).min(integer(&a["max_hp"], 1)));
                a["dnd5e"]["hit_dice_spent"] = json!(used + n);
            }
        }
        _ => return Err(bad("Action de création inconnue")),
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn gm() -> User {
        User {
            name: "MJ".into(),
            role: "gm".into(),
        }
    }
    #[test]
    fn creation_and_commit_are_validated_and_idempotent() {
        let mut w = demo();
        w["modules"] = json!({ID:{"enabled":true}});
        apply(&mut w, &gm(), &json!({"kind":"builder_draft_create"})).unwrap();
        let mut d = w["builder_drafts"][0].clone();
        d["data"]["name"] = json!("Éléonore");
        assert!(assemble(&w, &d).is_ok());
        d["data"]["scores"]["str"] = json!(18);
        assert!(assemble(&w, &d).is_err());
        d["data"]["scores"]["str"] = json!(15);
        w["builder_drafts"][0] = d.clone();
        let cmd = json!({"kind":"builder_commit","id":d["id"],"version":0});
        apply(&mut w, &gm(), &cmd).unwrap();
        apply(&mut w, &gm(), &cmd).unwrap();
        assert_eq!(w["actors"].as_array().unwrap().len(), 2);
        assert_eq!(w["actors"][1]["dnd5e"]["language"], "fr");
    }
    #[test]
    fn dice_are_locked_and_drafts_are_private() {
        let mut w = demo();
        w["modules"] = json!({ID:{"enabled":true}});
        apply(&mut w, &gm(), &json!({"kind":"builder_draft_create"})).unwrap();
        let id = w["builder_drafts"][0]["id"].clone();
        let cmd = json!({"kind":"builder_roll_scores","id":id});
        apply(&mut w, &gm(), &cmd).unwrap();
        let rolls = w["builder_drafts"][0]["rolled_scores"].clone();
        apply(&mut w, &gm(), &cmd).unwrap();
        assert_eq!(w["builder_drafts"][0]["rolled_scores"], rolls);
        let p = User {
            name: "Alice".into(),
            role: "player".into(),
        };
        assert!(apply(&mut w, &p, &cmd).is_err());
    }
}
