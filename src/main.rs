mod applescript;
mod mcp;
mod ops;
mod serve;
mod types;
mod url_scheme;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::types::{ActionResult, BuiltinList, ProjectItem, StatusFilter, TodoItem};

#[derive(Parser)]
#[command(
    name = "things-sak",
    version,
    about = "Things Swiss Army Knife — CLI and MCP server for Things 3"
)]
struct Cli {
    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Things URL scheme auth token (or set THINGS_SAK_AUTH_TOKEN).
    /// Required for update operations. Things → Settings → General → Enable Things URLs.
    #[arg(
        long,
        global = true,
        env = "THINGS_SAK_AUTH_TOKEN",
        hide_env_values = true
    )]
    auth_token: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new todo
    Add {
        title: String,
        #[arg(short, long)]
        notes: Option<String>,
        /// today, tomorrow, evening, anytime, someday, or a date (YYYY-MM-DD)
        #[arg(short, long)]
        when: Option<String>,
        /// Deadline date (YYYY-MM-DD)
        #[arg(short, long)]
        deadline: Option<String>,
        /// Tag to apply (repeatable)
        #[arg(short, long = "tag")]
        tags: Vec<String>,
        /// Project or area to add the todo to
        #[arg(short, long)]
        list: Option<String>,
        /// Heading within the target project
        #[arg(long)]
        heading: Option<String>,
        /// Checklist item (repeatable)
        #[arg(short, long = "checklist-item")]
        checklist_items: Vec<String>,
    },

    /// List todos from a built-in list
    List {
        #[arg(value_enum, default_value = "today")]
        list: BuiltinList,
        #[arg(short, long, value_enum, default_value = "open")]
        status: StatusFilter,
        /// Maximum number of todos to fetch
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Mark a todo as completed
    Done {
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },

    /// Update an existing todo
    Update {
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// Replacement notes
        #[arg(long)]
        notes: Option<String>,
        /// Text to append to notes
        #[arg(long)]
        append_notes: Option<String>,
        /// New schedule, or "none" to clear
        #[arg(short, long)]
        when: Option<String>,
        /// New deadline, or "none" to clear
        #[arg(short, long)]
        deadline: Option<String>,
        /// Replacement tag (repeatable, replaces all tags)
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Tag to add (repeatable)
        #[arg(long = "add-tag")]
        add_tags: Vec<String>,
        /// Project or area to move the todo into
        #[arg(short, long)]
        list: Option<String>,
    },

    /// Search todos by name across Inbox, Today, Anytime, Upcoming, Someday
    Search { query: String },

    /// Move a todo to a built-in list, project, or area
    Move {
        /// Destination: inbox, today, tomorrow, evening, anytime, someday, or a project/area title
        destination: String,
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },

    /// Remove a todo from its project
    Detach {
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },

    /// Project operations
    #[command(subcommand)]
    Project(ProjectCommand),

    /// List all tags
    Tags,

    /// List all areas
    Areas,

    /// Open Things at a list, or search for an item (UI navigation)
    Show { target: String },

    /// Start the MCP server on stdio
    Mcp,

    /// Start the MCP server over streamable HTTP
    Serve(serve::ServeArgs),
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// Create a new project
    Add {
        title: String,
        #[arg(short, long)]
        notes: Option<String>,
        #[arg(short, long)]
        area: Option<String>,
        #[arg(short, long)]
        when: Option<String>,
        #[arg(short, long)]
        deadline: Option<String>,
        #[arg(short, long = "tag")]
        tags: Vec<String>,
        /// Initial todo title (repeatable)
        #[arg(long = "todo")]
        todos: Vec<String>,
    },
    /// List projects
    List {
        #[arg(short, long)]
        area: Option<String>,
    },
    /// List todos in a project
    Todos {
        project: String,
        #[arg(short, long, value_enum, default_value = "open")]
        status: StatusFilter,
    },
    /// Move a project to an area
    Move { project: String, area: String },
    /// Remove a project from its area
    Detach { project: String },
}

fn non_empty(v: Vec<String>) -> Option<Vec<String>> {
    if v.is_empty() { None } else { Some(v) }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli).await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let json = cli.json;
    let token = cli.auth_token.as_deref();

    match cli.command {
        Command::Add {
            title,
            notes,
            when,
            deadline,
            tags,
            list,
            heading,
            checklist_items,
        } => {
            let result = ops::create_todo(ops::CreateTodo {
                title,
                notes,
                when,
                deadline,
                tags: non_empty(tags),
                list,
                heading,
                checklist_items: non_empty(checklist_items),
            })
            .await?;
            print_action(&result, json);
        }
        Command::List {
            list,
            status,
            limit,
        } => {
            let todos = ops::list_todos(list, status, limit).await?;
            print_todos(&todos, json);
        }
        Command::Done { name, id } => {
            let result = ops::complete_todo(id, name, token).await?;
            print_action(&result, json);
        }
        Command::Update {
            name,
            id,
            title,
            notes,
            append_notes,
            when,
            deadline,
            tags,
            add_tags,
            list,
        } => {
            let result = ops::update_todo(
                ops::UpdateTodo {
                    id,
                    name,
                    title,
                    notes,
                    append_notes,
                    when,
                    deadline,
                    tags: non_empty(tags),
                    add_tags: non_empty(add_tags),
                    list,
                    completed: None,
                    canceled: None,
                },
                token,
            )
            .await?;
            print_action(&result, json);
        }
        Command::Search { query } => {
            let todos = ops::search_todos(&query).await?;
            print_todos(&todos, json);
        }
        Command::Move {
            destination,
            name,
            id,
        } => {
            let result = ops::move_todo(id, name, &destination, token).await?;
            print_action(&result, json);
        }
        Command::Detach { name, id } => {
            let result = ops::remove_todo_from_project(id.as_deref(), name.as_deref()).await?;
            print_action(&result, json);
        }
        Command::Project(cmd) => match cmd {
            ProjectCommand::Add {
                title,
                notes,
                area,
                when,
                deadline,
                tags,
                todos,
            } => {
                let result = ops::create_project(ops::CreateProject {
                    title,
                    notes,
                    area,
                    when,
                    deadline,
                    tags: non_empty(tags),
                    todos: non_empty(todos),
                })
                .await?;
                print_action(&result, json);
            }
            ProjectCommand::List { area } => {
                let projects = ops::list_projects(area.as_deref()).await?;
                print_projects(&projects, json);
            }
            ProjectCommand::Todos { project, status } => {
                let todos = ops::get_project_todos(&project, status).await?;
                print_todos(&todos, json);
            }
            ProjectCommand::Move { project, area } => {
                let result = ops::move_project_to_area(&project, &area, token).await?;
                print_action(&result, json);
            }
            ProjectCommand::Detach { project } => {
                let result = ops::remove_project_from_area(&project).await?;
                print_action(&result, json);
            }
        },
        Command::Tags => {
            let tags = ops::list_tags().await?;
            print_names(&tags, json);
        }
        Command::Areas => {
            let areas = ops::list_areas().await?;
            print_names(&areas, json);
        }
        Command::Show { target } => {
            let result = ops::show(&target).await?;
            print_action(&result, json);
        }
        Command::Mcp => {
            mcp::run_stdio().await?;
        }
        Command::Serve(args) => {
            serve::run(args).await?;
        }
    }
    Ok(())
}

// ── Output formatting ─────────────────────────────────────────────

fn print_json<T: serde::Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

fn status_symbol(status: &str) -> &'static str {
    match status {
        "completed" => "✓",
        "canceled" => "✕",
        _ => "○",
    }
}

fn print_todos(todos: &[TodoItem], json: bool) {
    if json {
        print_json(&todos);
        return;
    }
    if todos.is_empty() {
        println!("No todos found.");
        return;
    }
    for t in todos {
        let mut line = format!("{} {}", status_symbol(&t.status), t.name);
        if let Some(p) = &t.project {
            line.push_str(&format!(" · {p}"));
        } else if let Some(a) = &t.area {
            line.push_str(&format!(" · {a}"));
        }
        if let Some(d) = &t.deadline {
            line.push_str(&format!(" · due {d}"));
        }
        if let Some(tags) = &t.tags {
            for tag in tags.split(", ").filter(|s| !s.is_empty()) {
                line.push_str(&format!(" #{tag}"));
            }
        }
        if let Some(l) = &t.list {
            line.push_str(&format!(" [{l}]"));
        }
        println!("{line}");
    }
}

fn print_projects(projects: &[ProjectItem], json: bool) {
    if json {
        print_json(&projects);
        return;
    }
    if projects.is_empty() {
        println!("No projects found.");
        return;
    }
    for p in projects {
        let mut line = format!("{} {}", status_symbol(&p.status), p.name);
        if let Some(a) = &p.area {
            line.push_str(&format!(" · {a}"));
        }
        println!("{line}");
    }
}

fn print_names(names: &[String], json: bool) {
    if json {
        print_json(&names);
        return;
    }
    if names.is_empty() {
        println!("None found.");
        return;
    }
    for name in names {
        println!("{name}");
    }
}

fn print_action(result: &ActionResult, json: bool) {
    if json {
        print_json(result);
        return;
    }
    match &result.id {
        Some(id) => println!("{} (id: {id})", result.message),
        None => println!("{}", result.message),
    }
}
