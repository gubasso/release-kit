//! The one bounded migration of a schema 1 configuration into the schema
//! 2 domains, over the authored text, so every comment survives.
//!
//! Schema 1 kept the technology, the forge, and the trunk under
//! `[project]`, the checkout mode, the style, and every opt-in under
//! `[landing]`, and the line prefix under `[setup]`. Schema 2 states each
//! under the domain that owns it. The migration moves each value with the
//! decor it carried, so a comment the operator wrote travels with its
//! line, renames the two checkout-mode values, and states the reporting
//! policy the older landing carried implicitly.
//!
//! A comment the template itself wrote is refreshed rather than carried:
//! a key whose vocabulary moved would otherwise stand beside a sentence
//! describing the key it replaced. The template's own comments are the
//! ones marked `# P:`, `# N:`, and `# F:`, and only those are replaced.
//!
//! SATISFIES project-profile:a-schema-one-configuration-migrates-in-place

use toml_edit::{DocumentMut, Item, Table, Value};

use crate::error::RkError;

/// The three class markers the authored template opens its comments with.
/// A trailing comment beginning with one of these is the template's own,
/// so a migration refreshes it; anything else is the operator's and
/// travels with its value.
const CLASS_MARKERS: [&str; 3] = ["# P:", "# N:", "# F:"];

/// Whether a trailing comment is the template's own.
fn templated(decor: Option<&str>) -> bool {
    decor.is_some_and(|text| {
        let trimmed = text.trim_start();
        CLASS_MARKERS
            .iter()
            .any(|marker| trimmed.starts_with(marker))
    })
}

/// The authored template's trailing comment for one schema 2 key path,
/// where the template carries one.
///
/// The template is a TOML skeleton whose values are substitution tokens,
/// so it is read line by line rather than parsed: a `[table]` header
/// names the path above each `key = TOKEN # comment` line below it.
fn template_comment(path: &[&str]) -> Option<String> {
    let text = crate::embedded::BLOCKS
        .get_file("target-config.toml.in")
        .and_then(include_dir::File::contents_utf8)?;
    let (key, table) = path.split_last()?;
    let wanted = table.join(".");
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            name.clone_into(&mut current);
            continue;
        }
        let Some((name, rest)) = trimmed.split_once(" = ") else {
            continue;
        };
        if name != *key || current != wanted {
            continue;
        }
        let comment = rest.find(" # ").map(|at| rest[at + 1..].to_owned());
        return comment.map(|comment| format!(" {comment}"));
    }
    None
}

/// Give the value at `path` the template's own comment, where the comment
/// it carries is the template's rather than the operator's.
fn refresh_comment(document: &mut DocumentMut, path: &[&str]) {
    let Some(comment) = template_comment(path) else {
        return;
    };
    let mut item = document.as_item_mut();
    for segment in path {
        if item.get(segment).is_none() {
            return;
        }
        item = &mut item[segment];
    }
    let Some(value) = item.as_value_mut() else {
        return;
    };
    let current = value.decor().suffix().and_then(|s| s.as_str());
    if current.is_none() || templated(current) {
        value.decor_mut().set_suffix(comment);
    }
}

/// Move `key` out of `from` into `to` under `name`, decor and all, where
/// `from` carries it.
fn move_key(from: &mut Table, key: &str, to: &mut Table, name: &str) -> Option<Value> {
    let item = from.remove(key)?;
    let value = item.into_value().ok()?;
    to.insert(name, Item::Value(value.clone()));
    Some(value)
}

/// Whether a value is the empty string, which schema 1 used for "detect".
fn empty(value: &Value) -> bool {
    value.as_str().is_some_and(str::is_empty)
}

/// A schema 1 configuration's text as schema 2, or the text unchanged
/// where it already states schema 2.
///
/// # Errors
///
/// Returns the reader's refusal for text that does not parse as TOML.
#[allow(
    clippy::too_many_lines,
    reason = "one pass moves every schema 1 key into the domain that owns it, and splitting it would separate a move from the decor it carries"
)]
pub fn to_schema_2(text: &str) -> Result<String, RkError> {
    let mut document = text
        .parse::<DocumentMut>()
        .map_err(|error| super::invalid(error.to_string()))?;
    if document.get("schema_version").and_then(Item::as_integer) != Some(1) {
        return Ok(text.to_owned());
    }
    document["schema_version"] = toml_edit::value(2);
    if let Some(item) = document.get_mut("schema_version")
        && let Some(value) = item.as_value_mut()
    {
        // The schema line keeps whatever decor the authored one carried.
        let old = text
            .parse::<DocumentMut>()
            .ok()
            .and_then(|old| old.get("schema_version").and_then(Item::as_value).cloned());
        if let Some(old) = old {
            *value.decor_mut() = old.decor().clone();
        }
    }

    let mut project = document
        .remove("project")
        .and_then(|item| item.into_table().ok())
        .unwrap_or_default();
    let mut landing = document
        .remove("landing")
        .and_then(|item| item.into_table().ok())
        .unwrap_or_default();

    // `[profile]`: the one technology becomes the sole entry of the list,
    // and the forge moves as it is. An empty value meant "detect" and
    // becomes an absent key.
    let mut profile = Table::new();
    if let Some(tech) = project
        .remove("tech")
        .and_then(|item| item.into_value().ok())
    {
        if !empty(&tech) {
            let mut list = toml_edit::Array::new();
            list.push(tech.as_str().unwrap_or_default());
            let mut value = Value::Array(list);
            *value.decor_mut() = tech.decor().clone();
            profile.insert("technologies", Item::Value(value));
            let mut release = Table::new();
            release.insert("mode", toml_edit::value("automatic"));
            release.insert(
                "driver",
                toml_edit::value(tech.as_str().unwrap_or_default()),
            );
            move_key(&mut landing, "style", &mut release, "style");
            let mut setup_table = document
                .remove("setup")
                .and_then(|item| item.into_table().ok())
                .unwrap_or_default();
            move_key(&mut setup_table, "line_prefix", &mut release, "line_prefix");
            document.insert("setup", Item::Table(setup_table));
            profile.insert("release", Item::Table(release));
        }
    } else {
        // No technology at all: the setup table still loses its prefix key
        // and the style key goes, because neither belongs to a profile with
        // no automatic release.
        if let Some(setup) = document.get_mut("setup").and_then(Item::as_table_mut) {
            setup.remove("line_prefix");
        }
        landing.remove("style");
    }
    if let Some(forge) = project
        .remove("forge")
        .and_then(|item| item.into_value().ok())
        && !empty(&forge)
    {
        profile.insert("forge", Item::Value(forge));
    }

    // `[git]`: the trunk, and the checkout mode with its values renamed.
    let mut git = Table::new();
    move_key(&mut project, "trunk", &mut git, "trunk");
    if let Some(mode) = landing
        .remove("workflow")
        .and_then(|item| item.into_value().ok())
    {
        let renamed = match mode.as_str() {
            Some("worktree") => Some("linked-worktree"),
            Some("branches") => Some("main-worktree"),
            _ => None,
        };
        let mut value = renamed.map_or_else(|| mode.clone(), Value::from);
        *value.decor_mut() = mode.decor().clone();
        git.insert("checkout_mode", Item::Value(value));
    }

    // `[capabilities]`: the opt-ins, and the reporting policy the older
    // landing carried without a key.
    let mut capabilities = Table::new();
    move_key(&mut landing, "nix", &mut capabilities, "nix_packaging");
    capabilities.insert("reporting_policy", toml_edit::value(true));
    move_key(&mut landing, "scorecard", &mut capabilities, "scorecard");
    move_key(
        &mut landing,
        "code_scanning",
        &mut capabilities,
        "code_scanning",
    );

    // Reassemble in the schema 2 order: the project table keeps its
    // remaining keys and its decor, the new tables follow it, and the
    // tables schema 2 keeps stand after them in their authored order.
    let mut rest: Vec<(String, Item)> = Vec::new();
    for name in ["security", "setup", "protection"] {
        if let Some(item) = document.remove(name) {
            rest.push((name.to_owned(), item));
        }
    }
    document.insert("project", Item::Table(project));
    for (name, table) in [
        ("profile", profile),
        ("git", git),
        ("capabilities", capabilities),
    ] {
        let mut table = table;
        table.set_implicit(table.is_empty());
        if name == "profile"
            && let Some(release) = table.get_mut("release").and_then(Item::as_table_mut)
        {
            release.set_implicit(release.is_empty());
        }
        document.insert(name, Item::Table(table));
    }
    for (name, item) in rest {
        document.insert(&name, item);
    }
    // Every key the migration writes states what it means now: a comment
    // the operator wrote stays, and the template's own is refreshed.
    for path in [
        &["project", "repo"][..],
        &["profile", "technologies"],
        &["profile", "forge"],
        &["profile", "release", "mode"],
        &["profile", "release", "driver"],
        &["profile", "release", "style"],
        &["profile", "release", "line_prefix"],
        &["git", "trunk"],
        &["git", "checkout_mode"],
        &["capabilities", "nix_packaging"],
        &["capabilities", "reporting_policy"],
        &["capabilities", "scorecard"],
        &["capabilities", "code_scanning"],
    ] {
        refresh_comment(&mut document, path);
    }
    Ok(document.to_string())
}

#[cfg(test)]
mod tests {
    use super::to_schema_2;

    /// Every moved key keeps its trailing comment, the mode values rename,
    /// the reporting policy appears, and the unmoved tables survive.
    #[test]
    fn a_schema_1_text_migrates_with_its_comments() {
        let old = "# heading\nschema_version = 1 # the schema\n\n[project]\nrepo = \"acme/widget\" # operator note\nforge = \"github\" # P: forge\ntech = \"rust\" # P: binding\ntrunk = \"main\" # N: trunk\n\n[landing]\nworkflow = \"worktree\" # P: mode\nstyle = \"lines\" # P: style\nnix = true # P: nix\nscorecard = false\ncode_scanning = \"semgrep\"\n\n[security]\ncontact = \"team\" # keep\n\n[setup]\nrequired_check = \"gate\"\nline_prefix = \"stable/\" # P: prefix\n";
        let migrated = to_schema_2(old).expect("migrates");
        let doc: toml::Table = migrated.parse().expect("the result parses");
        assert_eq!(doc["schema_version"].as_integer(), Some(2));
        assert!(migrated.contains("schema_version = 2 # the schema"));
        assert_eq!(doc["project"]["repo"].as_str(), Some("acme/widget"));
        assert!(doc["project"].get("tech").is_none());
        assert_eq!(
            doc["profile"]["technologies"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(doc["profile"]["forge"].as_str(), Some("github"));
        assert_eq!(
            doc["profile"]["release"]["mode"].as_str(),
            Some("automatic")
        );
        assert_eq!(doc["profile"]["release"]["driver"].as_str(), Some("rust"));
        assert_eq!(doc["profile"]["release"]["style"].as_str(), Some("lines"));
        assert!(
            migrated.contains("style = \"lines\" # P: trunk or lines; automatic alone"),
            "the template's own comment states what the key means now: {migrated}"
        );
        assert_eq!(
            doc["profile"]["release"]["line_prefix"].as_str(),
            Some("stable/")
        );
        assert!(migrated.contains("line_prefix = \"stable/\" # P: release-line branch prefix"));
        assert_eq!(doc["git"]["trunk"].as_str(), Some("main"));
        assert_eq!(
            doc["git"]["checkout_mode"].as_str(),
            Some("linked-worktree")
        );
        assert!(
            migrated.contains(
                "checkout_mode = \"linked-worktree\" # P: linked-worktree or main-worktree"
            ),
            "a renamed vocabulary takes the template's own sentence: {migrated}"
        );
        assert!(
            migrated.contains("repo = \"acme/widget\" # operator note"),
            "a comment the operator wrote travels with its value: {migrated}"
        );
        assert_eq!(doc["capabilities"]["nix_packaging"].as_bool(), Some(true));
        assert_eq!(
            doc["capabilities"]["reporting_policy"].as_bool(),
            Some(true)
        );
        assert_eq!(doc["capabilities"]["scorecard"].as_bool(), Some(false));
        assert_eq!(
            doc["capabilities"]["code_scanning"].as_str(),
            Some("semgrep")
        );
        assert!(doc.get("landing").is_none());
        assert!(doc["setup"].get("line_prefix").is_none());
        assert_eq!(doc["setup"]["required_check"].as_str(), Some("gate"));
        assert!(migrated.contains("contact = \"team\" # keep"));
        assert!(migrated.starts_with("# heading\n"));
        // Idempotent: a schema 2 text passes through unchanged.
        assert_eq!(to_schema_2(&migrated).expect("passes"), migrated);
    }

    /// A schema 1 file with an empty technology and forge, the "detect"
    /// answers, migrates to absent keys.
    #[test]
    fn an_empty_detect_answer_becomes_an_absent_key() {
        let migrated = to_schema_2(
            "schema_version = 1\n[project]\nforge = \"\"\ntech = \"\"\n[landing]\nworkflow = \"branches\"\n",
        )
        .expect("migrates");
        let doc: toml::Table = migrated.parse().expect("parses");
        assert!(doc.get("profile").is_none_or(|p| p.get("forge").is_none()));
        assert!(
            doc.get("profile")
                .is_none_or(|p| p.get("technologies").is_none())
        );
        assert_eq!(doc["git"]["checkout_mode"].as_str(), Some("main-worktree"));
    }
}
