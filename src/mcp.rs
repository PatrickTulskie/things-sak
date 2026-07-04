//! MCP server exposing Things operations as tools.

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

use crate::ops;
use crate::types::{BuiltinList, StatusFilter};

#[derive(Clone, Default)]
pub struct ThingsServer {
    /// Things URL scheme auth token, needed by update tools.
    pub auth_token: Option<String>,
}

fn ok_json<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let text =
        serde_json::to_string(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn tool_err(err: anyhow::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("{err:#}"))])
}

macro_rules! respond {
    ($result:expr) => {
        match $result {
            Ok(value) => ok_json(&value),
            Err(err) => Ok(tool_err(err)),
        }
    };
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListTodosArgs {
    /// Built-in list to read.
    pub list: BuiltinList,
    /// Filter by status (default: open).
    pub status: Option<StatusFilter>,
    /// Maximum number of todos to fetch, applied before the status filter (useful for logbook).
    pub limit: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SearchTodosArgs {
    /// Substring to match against todo names.
    pub query: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ProjectTodosArgs {
    /// Project title.
    pub project: String,
    /// Filter by status (default: open).
    pub status: Option<StatusFilter>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListProjectsArgs {
    /// Only list projects in this area.
    pub area: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TodoRefArgs {
    /// Todo id (preferred, from list/search results).
    pub id: Option<String>,
    /// Todo name, used when id is not known.
    pub name: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MoveTodoArgs {
    /// Todo id (preferred).
    pub id: Option<String>,
    /// Todo name, used when id is not known.
    pub name: Option<String>,
    /// inbox, today, tomorrow, evening, anytime, someday, or a project/area title.
    pub destination: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MoveProjectArgs {
    /// Project title.
    pub project: String,
    /// Area title to move the project into.
    pub area: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ProjectRefArgs {
    /// Project title.
    pub project: String,
}

#[tool_router]
impl ThingsServer {
    // ── Reads ──────────────────────────────────────────────────

    #[tool(
        description = "List todos from a built-in Things list (inbox, today, anytime, upcoming, someday, logbook, trash)",
        annotations(read_only_hint = true)
    )]
    pub async fn list_todos(
        &self,
        Parameters(args): Parameters<ListTodosArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::list_todos(args.list, args.status.unwrap_or_default(), args.limit).await)
    }

    #[tool(
        description = "Search todos by name across Inbox, Today, Anytime, Upcoming, and Someday",
        annotations(read_only_hint = true)
    )]
    pub async fn search_todos(
        &self,
        Parameters(args): Parameters<SearchTodosArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::search_todos(&args.query).await)
    }

    #[tool(
        description = "List todos inside a specific project",
        annotations(read_only_hint = true)
    )]
    pub async fn get_project_todos(
        &self,
        Parameters(args): Parameters<ProjectTodosArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::get_project_todos(&args.project, args.status.unwrap_or_default()).await)
    }

    #[tool(
        description = "List projects, optionally filtered by area",
        annotations(read_only_hint = true)
    )]
    pub async fn list_projects(
        &self,
        Parameters(args): Parameters<ListProjectsArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::list_projects(args.area.as_deref()).await)
    }

    #[tool(description = "List all areas", annotations(read_only_hint = true))]
    pub async fn list_areas(&self) -> Result<CallToolResult, ErrorData> {
        respond!(ops::list_areas().await)
    }

    #[tool(description = "List all tags", annotations(read_only_hint = true))]
    pub async fn list_tags(&self) -> Result<CallToolResult, ErrorData> {
        respond!(ops::list_tags().await)
    }

    // ── Writes ─────────────────────────────────────────────────

    #[tool(description = "Create a new todo in Things")]
    pub async fn create_todo(
        &self,
        Parameters(args): Parameters<ops::CreateTodo>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::create_todo(args).await)
    }

    #[tool(
        description = "Update an existing todo: title, notes, schedule, deadline, tags, project/area, completion"
    )]
    pub async fn update_todo(
        &self,
        Parameters(args): Parameters<ops::UpdateTodo>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::update_todo(args, self.auth_token.as_deref()).await)
    }

    #[tool(description = "Mark a todo as completed")]
    pub async fn complete_todo(
        &self,
        Parameters(args): Parameters<TodoRefArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::complete_todo(args.id, args.name, self.auth_token.as_deref()).await)
    }

    #[tool(
        description = "Move a todo to a built-in list (inbox, today, tomorrow, evening, anytime, someday) or into a project/area by title"
    )]
    pub async fn move_todo(
        &self,
        Parameters(args): Parameters<MoveTodoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(
            ops::move_todo(
                args.id,
                args.name,
                &args.destination,
                self.auth_token.as_deref()
            )
            .await
        )
    }

    #[tool(description = "Remove a todo from its project (the todo itself is kept)")]
    pub async fn remove_todo_from_project(
        &self,
        Parameters(args): Parameters<TodoRefArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::remove_todo_from_project(args.id.as_deref(), args.name.as_deref()).await)
    }

    #[tool(description = "Create a new project in Things")]
    pub async fn create_project(
        &self,
        Parameters(args): Parameters<ops::CreateProject>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::create_project(args).await)
    }

    #[tool(description = "Move a project into an area")]
    pub async fn move_project_to_area(
        &self,
        Parameters(args): Parameters<MoveProjectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(
            ops::move_project_to_area(&args.project, &args.area, self.auth_token.as_deref()).await
        )
    }

    #[tool(description = "Remove a project from its area (the project itself is kept)")]
    pub async fn remove_project_from_area(
        &self,
        Parameters(args): Parameters<ProjectRefArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        respond!(ops::remove_project_from_area(&args.project).await)
    }
}

#[tool_handler(
    name = "things-sak",
    instructions = "Tools for reading and writing Things 3 todos, projects, areas, and tags on the connected Mac. Reads return JSON arrays including item ids; prefer ids when updating. Todos have a schedule ('when') and an optional deadline — these are different fields."
)]
impl ServerHandler for ThingsServer {}

pub fn server_from_env() -> ThingsServer {
    ThingsServer {
        auth_token: std::env::var(crate::url_scheme::AUTH_TOKEN_ENV)
            .ok()
            .filter(|t| !t.is_empty()),
    }
}

pub async fn run_stdio() -> anyhow::Result<()> {
    let service = server_from_env().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
