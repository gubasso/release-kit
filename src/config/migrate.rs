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

use std::fmt::Write as _;
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
pub(crate) fn template_comment(path: &[&str]) -> Option<String> {
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
    // The comment standing above the key is read before the move: the
    // parser holds it on the source key, which the move discards, and
    // `project-profile:a-schema-one-configuration-migrates-in-place` asks
    // for every free-standing comment to survive.
    let carried = crate::config::key_comments(from, key);
    let item = from.remove(key)?;
    let value = item.into_value().ok()?;
    to.insert(name, Item::Value(value.clone()));
    if let Some(carried) = carried {
        crate::config::set_key_comments(to, name, &carried);
    }
    Some(value)
}

/// The key's value where schema 1 stated one, `None` where it stated the
/// empty "detect" answer.
///
/// A dropped detect answer may still carry a comment the operator wrote,
/// and `project-profile:a-schema-one-configuration-migrates-in-place` asks
/// for every free-standing comment to survive the migration. The comment
/// moves onto the table's header, which carries it further if the table
/// itself empties.
fn dropped_or_kept(table: &mut Table, key: &str) -> (Option<Value>, Option<String>) {
    let carried = crate::config::key_comments(table, key);
    let empty_answer = table.get(key).and_then(Item::as_value).is_some_and(empty);
    if empty_answer {
        crate::config::take_comments(table, key);
        return (None, None);
    }
    let value = table.remove(key).and_then(|item| item.into_value().ok());
    (value, carried)
}

/// Every key schema 1's `[landing]` table could carry.
const LANDING_KEYS: [&str; 5] = ["workflow", "style", "nix", "scorecard", "code_scanning"];

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
    // A file stating schema 1 must have a schema 1 shape. The strict
    // reader never sees this text, so a table schema 1 did not have, or a
    // legacy container that is not a table, has to refuse here: writing
    // the generated table over it would replace the operator's content
    // with a valid file and report success.
    for name in ["profile", "git", "capabilities"] {
        if document.get(name).is_some() {
            return Err(super::invalid(format!(
                "[{name}] is a schema 2 table and this file states schema_version = 1; set schema_version = 2, or remove the table"
            )));
        }
    }
    for name in ["project", "landing", "setup"] {
        if document
            .get(name)
            .is_some_and(|item| item.as_table().is_none())
        {
            return Err(super::invalid(format!(
                "{name} must be a table in a schema 1 file"
            )));
        }
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
    let (tech, tech_comments) = dropped_or_kept(&mut project, "tech");
    if let Some(tech) = tech {
        let mut list = toml_edit::Array::new();
        list.push(tech.as_str().unwrap_or_default());
        let mut value = Value::Array(list);
        *value.decor_mut() = tech.decor().clone();
        profile.insert("technologies", Item::Value(value));
        if let Some(carried) = &tech_comments {
            crate::config::set_key_comments(&mut profile, "technologies", carried);
        }
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
    } else {
        // No technology, whether the key is absent or the schema 1 detect
        // value: the setup table still loses its prefix key and the style
        // key goes, because neither belongs to a profile with no automatic
        // release.
        if let Some(setup) = document.get_mut("setup").and_then(Item::as_table_mut) {
            crate::config::take_comments(setup, "line_prefix");
        }
        crate::config::take_comments(&mut landing, "style");
    }
    let (forge, forge_comments) = dropped_or_kept(&mut project, "forge");
    if let Some(forge) = forge {
        profile.insert("forge", Item::Value(forge));
        if let Some(carried) = &forge_comments {
            crate::config::set_key_comments(&mut profile, "forge", carried);
        }
    }

    // `[git]`: the trunk, and the checkout mode with its values renamed.
    let mut git = Table::new();
    move_key(&mut project, "trunk", &mut git, "trunk");
    // The checkout mode renames its key and its vocabulary at once, so it
    // cannot go through `move_key`; it reads the source key's comments the
    // same way, because a rename is still a move to the operator.
    let mode_comments = crate::config::key_comments(&landing, "workflow");
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
        if let Some(carried) = &mode_comments {
            crate::config::set_key_comments(&mut git, "checkout_mode", carried);
        }
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

    // The `[landing]` table has no schema 2 successor, so it is discarded
    // with whatever is left in it. A key still standing in it is one this
    // migration does not know, and silently dropping it would take the
    // unknown-key policy off the schema 1 path: the strict reader never
    // sees the original text, so this is where `target-config:an-unknown-key-refuses`
    // has to hold.
    if let Some(unknown) = landing.iter().map(|(key, _)| key).next() {
        let mut message = format!("landing.{unknown} is not a key this migration knows");
        if let Some(nearest) = crate::config::nearest_known(unknown, &LANDING_KEYS) {
            let _ = write!(message, "; nearest known key: landing.{nearest}");
        }
        return Err(super::invalid(message));
    }
    // A comment still standing on its header, including one carried off a
    // key this migration dropped, outlives it.
    let orphaned = crate::config::take_header_comments(&mut landing);

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
    // A schema 1 file whose project keys all moved or dropped leaves an
    // empty table, which
    // `target-config:an-unanswered-key-is-absent-and-not-empty` asks the
    // writer to leave out. Pruning it rather than hiding it keeps every
    // comment the header carried, which hiding would suppress with it.
    crate::config::prune_empty_tables(&mut document, &["project"]);
    if let Some(orphaned) = orphaned {
        crate::config::place_carried(&mut document, &orphaned);
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
        assert!(
            !migrated.contains("[project]"),
            "a table every one of whose keys dropped goes with them: {migrated}"
        );
    }

    /// SATISFIES project-profile:a-schema-one-configuration-migrates-in-place
    /// The migrated file loses the header whose every key moved or
    /// dropped, and keeps the comment that header carried.
    #[test]
    fn an_emptied_project_header_goes_and_its_comment_stays() {
        let migrated = to_schema_2(
            "schema_version = 1\n\n# the operator's note\n[project]\nforge = \"\"\ntech = \"\"\n\n[security]\ncontact = \"team\"\n",
        )
        .expect("migrates");
        assert!(
            !migrated.contains("[project]"),
            "a table every one of whose keys dropped goes with them: {migrated}"
        );
        assert!(
            migrated.contains("# the operator's note"),
            "the comment the header carried survives: {migrated}"
        );
        crate::config::parse(&migrated).expect("the strict schema 2 reader accepts it");
    }

    /// SATISFIES project-profile:a-schema-one-configuration-migrates-in-place
    /// The detect answer and the absent key take the same cleanup path. A
    /// schema 1 file could state `tech = ""` beside a release-line prefix
    /// and a style, and both keys belong to an automatic release the
    /// migrated profile does not have. Leaving either behind writes a
    /// schema 2 file the strict reader then rejects.
    #[test]
    fn an_empty_technology_still_clears_the_automatic_release_keys() {
        let migrated = to_schema_2(
            "schema_version = 1\n[project]\ntech = \"\"\nforge = \"github\"\nrepo = \"acme/widget\"\n[landing]\nstyle = \"lines\"\n[setup]\nline_prefix = \"stable/\"\nrequired_check = \"gate\"\n",
        )
        .expect("migrates");
        let doc: toml::Table = migrated.parse().expect("parses");
        assert!(
            doc["setup"].get("line_prefix").is_none(),
            "the prefix belongs to an automatic release: {migrated}"
        );
        assert!(
            doc.get("profile")
                .is_none_or(|profile| profile.get("release").is_none()),
            "no technology means no release intent: {migrated}"
        );
        assert!(doc.get("landing").is_none(), "{migrated}");
        assert_eq!(doc["setup"]["required_check"].as_str(), Some("gate"));
        crate::config::parse(&migrated).expect("the strict schema 2 reader accepts it");
    }

    /// SATISFIES project-profile:a-schema-one-configuration-migrates-in-place
    /// A schema 1 key the migration drops takes the template's own comment
    /// with it and leaves the operator's behind. Every free-standing
    /// comment survives, which is what the rule asks for.
    #[test]
    fn a_dropped_schema_1_key_keeps_the_operators_comment() {
        let migrated = to_schema_2(concat!(
            "schema_version = 1\n\n[project]\nrepo = \"acme/widget\"\n",
            "# this project has no forge yet\n",
            "forge = \"\" # P: forge\n",
            "# and no binding release-kit knows\n",
            "tech = \"\" # P: binding\n\n",
            "[landing]\n# the style we used to ask for\nstyle = \"lines\"\n\n",
            "[setup]\nrequired_check = \"gate\"\n",
            "# the prefix we used to ask for\nline_prefix = \"stable/\"\n"
        ))
        .expect("migrates");
        for note in [
            "# this project has no forge yet",
            "# and no binding release-kit knows",
            "# the style we used to ask for",
            "# the prefix we used to ask for",
        ] {
            assert!(
                migrated.contains(note),
                "authored text survives the migration: {note}: {migrated}"
            );
        }
        assert!(!migrated.contains("forge ="), "{migrated}");
        assert!(!migrated.contains("tech ="), "{migrated}");
        assert!(!migrated.contains("style ="), "{migrated}");
        assert!(!migrated.contains("line_prefix ="), "{migrated}");
        crate::config::parse(&migrated).expect("the strict schema 2 reader accepts it");
    }

    /// SATISFIES project-profile:a-schema-one-configuration-migrates-in-place
    /// A comment standing above a key the migration moves travels with the
    /// value it describes, rather than staying beside a key that is gone.
    #[test]
    fn a_moved_key_takes_the_comment_above_it() {
        let migrated = to_schema_2(concat!(
            "schema_version = 1\n\n[project]\nrepo = \"acme/widget\"\n",
            "# the one binding this project releases from\ntech = \"rust\"\n",
            "forge = \"github\"\n",
            "# why this branch is fixed\ntrunk = \"main\"\n\n",
            "[landing]\n# why this project takes lines\nstyle = \"lines\"\n",
            "# why topics use the main checkout\nworkflow = \"branches\"\n"
        ))
        .expect("migrates");
        for note in [
            "# the one binding this project releases from",
            "# why this branch is fixed",
            "# why this project takes lines",
            "# why topics use the main checkout",
        ] {
            assert!(migrated.contains(note), "{note}: {migrated}");
        }
        let doc: toml::Table = migrated.parse().expect("parses");
        assert_eq!(doc["git"]["trunk"].as_str(), Some("main"));
        assert_eq!(doc["profile"]["release"]["style"].as_str(), Some("lines"));
        assert_eq!(doc["git"]["checkout_mode"].as_str(), Some("main-worktree"));
        crate::config::parse(&migrated).expect("the strict schema 2 reader accepts it");
    }

    /// SATISFIES target-config:an-unknown-key-refuses
    /// The strict reader never sees the schema 1 text, so a key the
    /// migration does not know refuses here. Dropping it silently would
    /// accept a typo and land the default behaviour under it.
    #[test]
    fn an_unknown_schema_1_landing_key_refuses_by_name() {
        let refusal = to_schema_2("schema_version = 1\n[landing]\nworkflo = \"branches\"\n")
            .expect_err("an unknown key refuses")
            .to_string();
        assert!(refusal.contains("workflo"), "{refusal}");
        assert!(
            refusal.contains("nearest known key: landing.workflow"),
            "{refusal}"
        );
    }

    /// SATISFIES target-config:an-unknown-key-refuses
    /// A file that states schema 1 must have a schema 1 shape. The
    /// generated tables would otherwise be written over whatever stood
    /// where they go, and the result would read as valid.
    #[test]
    fn a_schema_1_file_with_a_schema_2_shape_refuses() {
        for (text, named) in [
            (
                "schema_version = 1\n[profile]\nforg = \"github\"\n",
                "[profile]",
            ),
            ("schema_version = 1\n[git]\ntrunk = \"main\"\n", "[git]"),
            (
                "schema_version = 1\n[capabilities]\nscorecard = true\n",
                "[capabilities]",
            ),
            ("schema_version = 1\nproject = \"acme/widget\"\n", "project"),
            ("schema_version = 1\nlanding = 3\n", "landing"),
            (
                "schema_version = 1\nsetup = \"operator value\"\n[project]\ntech = \"rust\"\n",
                "setup",
            ),
        ] {
            let refusal = to_schema_2(text)
                .expect_err("a shape schema 1 never had refuses")
                .to_string();
            assert!(refusal.contains(named), "{named}: {refusal}");
        }
    }
}
