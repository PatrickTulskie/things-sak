use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TodoItem {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// Which built-in list the todo was found in (search results only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectItem {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActionResult {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    #[default]
    Open,
    Completed,
    Canceled,
    All,
}

impl StatusFilter {
    pub fn matches(&self, status: &str) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Open => status == "open",
            StatusFilter::Completed => status == "completed",
            StatusFilter::Canceled => status == "canceled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BuiltinList {
    Inbox,
    Today,
    Anytime,
    Upcoming,
    Someday,
    Logbook,
    Trash,
}

impl BuiltinList {
    pub fn as_things_name(&self) -> &'static str {
        match self {
            BuiltinList::Inbox => "Inbox",
            BuiltinList::Today => "Today",
            BuiltinList::Anytime => "Anytime",
            BuiltinList::Upcoming => "Upcoming",
            BuiltinList::Someday => "Someday",
            BuiltinList::Logbook => "Logbook",
            BuiltinList::Trash => "Trash",
        }
    }
}
