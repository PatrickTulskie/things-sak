//! High-level Things operations shared by the CLI and the MCP server.
//!
//! Reads go through read-only AppleScript queries; writes go through the
//! official `things:///` URL scheme. The only AppleScript writes are the few
//! operations the URL scheme cannot express (move to Inbox, detach from
//! project/area) — these still use the official scripting API, never the
//! Things database.

use std::time::Duration;

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::applescript::{self, SEP_PREAMBLE, opt, parse_records, quote, tell_things};
use crate::types::{ActionResult, BuiltinList, ProjectItem, StatusFilter, TodoItem};
use crate::url_scheme;

/// AppleScript snippet appending one todo record (8 fields) to `out`.
/// Expects `fs`/`rs` from SEP_PREAMBLE and the todo bound to `t`.
const TODO_RECORD: &str = r#"
set dl to due date of t
if dl is missing value then
  set dlStr to ""
else
  set dlStr to (year of dl as string) & "-" & (text -2 thru -1 of ("0" & ((month of dl as integer) as string))) & "-" & (text -2 thru -1 of ("0" & ((day of dl) as string)))
end if
set projName to ""
try
  set projName to name of project of t
end try
set areaName to ""
try
  set areaName to name of area of t
end try
set out to out & (id of t) & fs & (name of t) & fs & ((status of t) as string) & fs & (notes of t) & fs & dlStr & fs & (tag names of t) & fs & projName & fs & areaName"#;

fn todo_from_fields(fields: &[String]) -> Option<TodoItem> {
    if fields.len() < 8 {
        return None;
    }
    Some(TodoItem {
        id: fields[0].clone(),
        name: fields[1].clone(),
        status: fields[2].clone(),
        notes: opt(&fields[3]),
        deadline: opt(&fields[4]),
        tags: opt(&fields[5]),
        project: opt(&fields[6]),
        area: opt(&fields[7]),
        list: fields.get(8).and_then(|f| opt(f)),
    })
}

// ── Reads ──────────────────────────────────────────────────────────

pub async fn list_todos(
    list: BuiltinList,
    status: StatusFilter,
    limit: Option<usize>,
) -> Result<Vec<TodoItem>> {
    let list_ref = format!("to dos of list {}", quote(list.as_things_name()));
    let limit_clause = match limit {
        Some(n) => format!(
            "if (count of theTodos) > {n} then set theTodos to items 1 thru {n} of theTodos"
        ),
        None => String::new(),
    };
    let script = tell_things(&format!(
        "{SEP_PREAMBLE}
set theTodos to {list_ref}
{limit_clause}
set out to \"\"
repeat with t in theTodos
{TODO_RECORD} & rs
end repeat
return out"
    ));

    let output = applescript::run(&script).await?;
    Ok(parse_records(&output)
        .iter()
        .filter_map(|f| todo_from_fields(f))
        .filter(|t| status.matches(&t.status))
        .collect())
}

pub async fn search_todos(query: &str) -> Result<Vec<TodoItem>> {
    let q = quote(query);
    let script = tell_things(&format!(
        "{SEP_PREAMBLE}
set out to \"\"
repeat with listName in {{\"Inbox\", \"Today\", \"Anytime\", \"Upcoming\", \"Someday\"}}
  try
    set matches to (to dos of list (contents of listName) whose name contains {q})
    repeat with t in matches
{TODO_RECORD} & fs & (contents of listName) & rs
    end repeat
  end try
end repeat
return out"
    ));

    let output = applescript::run(&script).await?;
    Ok(parse_records(&output)
        .iter()
        .filter_map(|f| todo_from_fields(f))
        .collect())
}

pub async fn get_project_todos(project: &str, status: StatusFilter) -> Result<Vec<TodoItem>> {
    let script = tell_things(&format!(
        "{SEP_PREAMBLE}
set theTodos to to dos of project {}
set out to \"\"
repeat with t in theTodos
{TODO_RECORD} & rs
end repeat
return out",
        quote(project)
    ));

    let output = applescript::run(&script).await?;
    Ok(parse_records(&output)
        .iter()
        .filter_map(|f| todo_from_fields(f))
        .filter(|t| status.matches(&t.status))
        .collect())
}

pub async fn list_projects(area: Option<&str>) -> Result<Vec<ProjectItem>> {
    let source = match area {
        Some(a) => format!("projects of area {}", quote(a)),
        None => "projects".to_string(),
    };
    let script = tell_things(&format!(
        "{SEP_PREAMBLE}
set theProjects to {source}
set out to \"\"
repeat with p in theProjects
  set areaName to \"\"
  try
    set areaName to name of area of p
  end try
  set out to out & (id of p) & fs & (name of p) & fs & ((status of p) as string) & fs & (notes of p) & fs & areaName & rs
end repeat
return out"
    ));

    let output = applescript::run(&script).await?;
    Ok(parse_records(&output)
        .iter()
        .filter(|f| f.len() >= 5)
        .map(|f| ProjectItem {
            id: f[0].clone(),
            name: f[1].clone(),
            status: f[2].clone(),
            notes: opt(&f[3]),
            area: opt(&f[4]),
        })
        .collect())
}

async fn list_names(kind: &str) -> Result<Vec<String>> {
    let script = tell_things(&format!(
        "set fs to character id 31
set AppleScript's text item delimiters to fs
return (name of every {kind}) as string"
    ));
    let output = applescript::run(&script).await?;
    Ok(output
        .split(applescript::FIELD_SEP)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

pub async fn list_tags() -> Result<Vec<String>> {
    list_names("tag").await
}

pub async fn list_areas() -> Result<Vec<String>> {
    list_names("area").await
}

// ── ID resolution ──────────────────────────────────────────────────

pub async fn resolve_todo_id(id: Option<&str>, name: Option<&str>) -> Result<String> {
    if let Some(id) = id {
        return Ok(id.to_string());
    }
    let Some(name) = name else {
        bail!("either an id or a name is required");
    };
    let script = tell_things(&format!("return id of to do named {}", quote(name)));
    applescript::run(&script)
        .await
        .map_err(|e| anyhow::anyhow!("could not find todo named \"{name}\": {e}"))
}

pub async fn resolve_project_id(id: Option<&str>, name: Option<&str>) -> Result<String> {
    if let Some(id) = id {
        return Ok(id.to_string());
    }
    let Some(name) = name else {
        bail!("either an id or a name is required");
    };
    let script = tell_things(&format!("return id of project named {}", quote(name)));
    applescript::run(&script)
        .await
        .map_err(|e| anyhow::anyhow!("could not find project named \"{name}\": {e}"))
}

/// Best-effort lookup of a just-created item's id. `open -g` returns before
/// Things processes the URL, so poll briefly.
async fn poll_new_id(kind: &str, name: &str) -> Option<String> {
    let script = tell_things(&format!("return id of {kind} named {}", quote(name)));
    for _ in 0..25 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if let Ok(id) = applescript::run(&script).await {
            return Some(id);
        }
    }
    None
}

// ── Writes (URL scheme) ────────────────────────────────────────────

#[derive(Debug, Default, Clone, Deserialize, schemars::JsonSchema)]
pub struct CreateTodo {
    /// Title of the new todo.
    pub title: String,
    /// Notes body (max 10,000 characters).
    pub notes: Option<String>,
    /// Schedule: today, tomorrow, evening, anytime, someday, or a date like 2026-07-10.
    pub when: Option<String>,
    /// Deadline date (YYYY-MM-DD).
    pub deadline: Option<String>,
    /// Tag titles to apply (tags must already exist in Things).
    pub tags: Option<Vec<String>>,
    /// Title of a project or area to add the todo to.
    pub list: Option<String>,
    /// Heading within the target project.
    pub heading: Option<String>,
    /// Checklist items to add (max 100).
    pub checklist_items: Option<Vec<String>>,
}

pub async fn create_todo(args: CreateTodo) -> Result<ActionResult> {
    let mut params: Vec<(&str, String)> = vec![("title", args.title.clone())];
    if let Some(v) = &args.notes {
        params.push(("notes", v.clone()));
    }
    if let Some(v) = &args.when {
        params.push(("when", v.clone()));
    }
    if let Some(v) = &args.deadline {
        params.push(("deadline", v.clone()));
    }
    if let Some(tags) = &args.tags
        && !tags.is_empty()
    {
        params.push(("tags", tags.join(",")));
    }
    if let Some(v) = &args.list {
        params.push(("list", v.clone()));
    }
    if let Some(v) = &args.heading {
        params.push(("heading", v.clone()));
    }
    if let Some(items) = &args.checklist_items
        && !items.is_empty()
    {
        params.push(("checklist-items", items.join("\n")));
    }

    let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    url_scheme::open("add", &borrowed).await?;

    let id = poll_new_id("to do", &args.title).await;
    Ok(ActionResult {
        message: format!("Created todo: {}", args.title),
        id,
    })
}

#[derive(Debug, Default, Clone, Deserialize, schemars::JsonSchema)]
pub struct UpdateTodo {
    /// Todo id (preferred). If omitted, `name` is used to find the todo.
    pub id: Option<String>,
    /// Current todo name, used to look up the id when `id` is omitted.
    pub name: Option<String>,
    /// New title.
    pub title: Option<String>,
    /// Replacement notes body.
    pub notes: Option<String>,
    /// Text to append to the existing notes.
    pub append_notes: Option<String>,
    /// New schedule: today, tomorrow, evening, anytime, someday, a date, or "none" to clear.
    pub when: Option<String>,
    /// New deadline (YYYY-MM-DD), or "none" to clear.
    pub deadline: Option<String>,
    /// Replacement tag titles (replaces all existing tags).
    pub tags: Option<Vec<String>>,
    /// Tag titles to add to the existing ones.
    pub add_tags: Option<Vec<String>>,
    /// Title of a project or area to move the todo into.
    pub list: Option<String>,
    /// Mark completed (true) or back to open (false).
    pub completed: Option<bool>,
    /// Mark canceled (true) or back to open (false).
    pub canceled: Option<bool>,
}

fn none_clears(value: &str) -> &str {
    if value.eq_ignore_ascii_case("none") {
        ""
    } else {
        value
    }
}

pub async fn update_todo(args: UpdateTodo, auth_token: Option<&str>) -> Result<ActionResult> {
    let token = url_scheme::auth_token(auth_token)?;
    let id = resolve_todo_id(args.id.as_deref(), args.name.as_deref()).await?;

    let mut params: Vec<(&str, String)> = vec![("id", id.clone()), ("auth-token", token)];
    if let Some(v) = &args.title {
        params.push(("title", v.clone()));
    }
    if let Some(v) = &args.notes {
        params.push(("notes", v.clone()));
    }
    if let Some(v) = &args.append_notes {
        params.push(("append-notes", v.clone()));
    }
    if let Some(v) = &args.when {
        params.push(("when", none_clears(v).to_string()));
    }
    if let Some(v) = &args.deadline {
        params.push(("deadline", none_clears(v).to_string()));
    }
    if let Some(tags) = &args.tags {
        params.push(("tags", tags.join(",")));
    }
    if let Some(tags) = &args.add_tags
        && !tags.is_empty()
    {
        params.push(("add-tags", tags.join(",")));
    }
    if let Some(v) = &args.list {
        params.push(("list", v.clone()));
    }
    if let Some(v) = args.completed {
        params.push(("completed", v.to_string()));
    }
    if let Some(v) = args.canceled {
        params.push(("canceled", v.to_string()));
    }

    if params.len() == 2 {
        bail!("no updates specified");
    }

    let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    url_scheme::open("update", &borrowed).await?;

    let label = args.name.as_deref().unwrap_or(&id);
    Ok(ActionResult {
        message: format!("Updated todo: {label}"),
        id: Some(id),
    })
}

pub async fn complete_todo(
    id: Option<String>,
    name: Option<String>,
    auth_token: Option<&str>,
) -> Result<ActionResult> {
    let label = name.clone().or_else(|| id.clone()).unwrap_or_default();
    let result = update_todo(
        UpdateTodo {
            id,
            name,
            completed: Some(true),
            ..Default::default()
        },
        auth_token,
    )
    .await?;
    Ok(ActionResult {
        message: format!("Completed todo: {label}"),
        id: result.id,
    })
}

/// Move a todo to a built-in list or schedule. Everything except Inbox maps
/// to the URL scheme `when` parameter; Inbox has no scheme equivalent, so it
/// uses the official AppleScript `move` command.
pub async fn move_todo(
    id: Option<String>,
    name: Option<String>,
    destination: &str,
    auth_token: Option<&str>,
) -> Result<ActionResult> {
    let dest = destination.to_lowercase();
    match dest.as_str() {
        "inbox" => {
            let todo_id = resolve_todo_id(id.as_deref(), name.as_deref()).await?;
            let script = tell_things(&format!(
                "move to do id {} to list \"Inbox\"",
                quote(&todo_id)
            ));
            applescript::run(&script).await?;
            Ok(ActionResult {
                message: "Moved todo to Inbox".to_string(),
                id: Some(todo_id),
            })
        }
        "today" | "tomorrow" | "evening" | "anytime" | "someday" => {
            let result = update_todo(
                UpdateTodo {
                    id,
                    name,
                    when: Some(dest.clone()),
                    ..Default::default()
                },
                auth_token,
            )
            .await?;
            Ok(ActionResult {
                message: format!("Moved todo to {dest}"),
                id: result.id,
            })
        }
        // Anything else is a project or area title.
        _ => {
            let result = update_todo(
                UpdateTodo {
                    id,
                    name,
                    list: Some(destination.to_string()),
                    ..Default::default()
                },
                auth_token,
            )
            .await?;
            Ok(ActionResult {
                message: format!("Moved todo to \"{destination}\""),
                id: result.id,
            })
        }
    }
}

/// Detach a todo from its project. No URL scheme equivalent; uses the
/// official AppleScript API (read-modify on the relationship, not the DB).
pub async fn remove_todo_from_project(
    id: Option<&str>,
    name: Option<&str>,
) -> Result<ActionResult> {
    let todo_id = resolve_todo_id(id, name).await?;
    let script = tell_things(&format!("delete project of to do id {}", quote(&todo_id)));
    applescript::run(&script).await?;
    Ok(ActionResult {
        message: "Removed todo from its project".to_string(),
        id: Some(todo_id),
    })
}

#[derive(Debug, Default, Clone, Deserialize, schemars::JsonSchema)]
pub struct CreateProject {
    /// Title of the new project.
    pub title: String,
    /// Notes body.
    pub notes: Option<String>,
    /// Title of the area to create the project in.
    pub area: Option<String>,
    /// Schedule: today, tomorrow, evening, anytime, someday, or a date.
    pub when: Option<String>,
    /// Deadline date (YYYY-MM-DD).
    pub deadline: Option<String>,
    /// Tag titles to apply.
    pub tags: Option<Vec<String>>,
    /// Initial todo titles to create inside the project.
    pub todos: Option<Vec<String>>,
}

pub async fn create_project(args: CreateProject) -> Result<ActionResult> {
    let mut params: Vec<(&str, String)> = vec![("title", args.title.clone())];
    if let Some(v) = &args.notes {
        params.push(("notes", v.clone()));
    }
    if let Some(v) = &args.area {
        params.push(("area", v.clone()));
    }
    if let Some(v) = &args.when {
        params.push(("when", v.clone()));
    }
    if let Some(v) = &args.deadline {
        params.push(("deadline", v.clone()));
    }
    if let Some(tags) = &args.tags
        && !tags.is_empty()
    {
        params.push(("tags", tags.join(",")));
    }
    if let Some(todos) = &args.todos
        && !todos.is_empty()
    {
        params.push(("to-dos", todos.join("\n")));
    }

    let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    url_scheme::open("add-project", &borrowed).await?;

    let id = poll_new_id("project", &args.title).await;
    Ok(ActionResult {
        message: format!("Created project: {}", args.title),
        id,
    })
}

pub async fn move_project_to_area(
    project: &str,
    area: &str,
    auth_token: Option<&str>,
) -> Result<ActionResult> {
    let token = url_scheme::auth_token(auth_token)?;
    let id = resolve_project_id(None, Some(project)).await?;
    url_scheme::open(
        "update-project",
        &[
            ("id", id.as_str()),
            ("auth-token", token.as_str()),
            ("area", area),
        ],
    )
    .await?;
    Ok(ActionResult {
        message: format!("Moved project \"{project}\" to area \"{area}\""),
        id: Some(id),
    })
}

/// Detach a project from its area. No URL scheme equivalent; uses the
/// official AppleScript API.
pub async fn remove_project_from_area(project: &str) -> Result<ActionResult> {
    let id = resolve_project_id(None, Some(project)).await?;
    let script = tell_things(&format!("delete area of project id {}", quote(&id)));
    applescript::run(&script).await?;
    Ok(ActionResult {
        message: format!("Removed project \"{project}\" from its area"),
        id: Some(id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_clears_maps_none_keyword_to_empty() {
        assert_eq!(none_clears("none"), "");
        assert_eq!(none_clears("NONE"), "");
        assert_eq!(none_clears("2026-07-10"), "2026-07-10");
    }

    #[test]
    fn todo_from_fields_maps_columns() {
        let fields: Vec<String> = [
            "id1",
            "Buy milk",
            "open",
            "",
            "2026-07-10",
            "errand",
            "Groceries",
            "",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let todo = todo_from_fields(&fields).unwrap();
        assert_eq!(todo.id, "id1");
        assert_eq!(todo.notes, None);
        assert_eq!(todo.deadline.as_deref(), Some("2026-07-10"));
        assert_eq!(todo.project.as_deref(), Some("Groceries"));
        assert_eq!(todo.area, None);
        assert_eq!(todo.list, None);
    }

    #[test]
    fn todo_from_fields_rejects_short_records() {
        assert!(
            todo_from_fields(&[
                "only".to_string(),
                "three".to_string(),
                "fields".to_string()
            ])
            .is_none()
        );
    }

    #[test]
    fn status_filter_matches() {
        assert!(StatusFilter::Open.matches("open"));
        assert!(!StatusFilter::Open.matches("completed"));
        assert!(StatusFilter::All.matches("canceled"));
    }
}

/// Open Things and navigate to a list, todo, or search query (things:///show).
pub async fn show(target: &str) -> Result<ActionResult> {
    let builtin = [
        "inbox",
        "today",
        "anytime",
        "upcoming",
        "someday",
        "logbook",
        "tomorrow",
        "deadlines",
    ];
    let params: Vec<(&str, &str)> = if builtin.contains(&target.to_lowercase().as_str()) {
        vec![("id", target)]
    } else {
        vec![("query", target)]
    };
    url_scheme::open("show", &params).await?;
    Ok(ActionResult {
        message: format!("Opened Things at: {target}"),
        id: None,
    })
}
