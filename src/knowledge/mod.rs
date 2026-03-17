pub mod loader;
pub mod indexer;
pub mod platform;

use serde::Deserialize;

/// Risk level of a command
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Moderate,
    Dangerous,
}

/// A single argument in a command template
#[derive(Debug, Clone, Deserialize)]
pub struct CommandArg {
    pub name: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub description: String,
}

/// An example showing natural language input → filled command
#[derive(Debug, Clone, Deserialize)]
pub struct CommandExample {
    pub input: String,
    pub filled: String,
}

/// A single command entry in the knowledge base
#[derive(Debug, Clone, Deserialize)]
pub struct CommandEntry {
    pub name: String,
    pub binary: String,
    pub template: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub examples: Vec<CommandExample>,
    #[serde(default = "default_platforms")]
    pub platforms: Vec<String>,
    #[serde(default = "default_risk")]
    pub risk: Risk,
    #[serde(default)]
    pub args: Vec<CommandArg>,
}

fn default_platforms() -> Vec<String> {
    vec!["linux".into(), "macos".into(), "windows".into()]
}

fn default_risk() -> Risk {
    Risk::Safe
}

/// TOML file structure: contains a list of command entries
#[derive(Debug, Deserialize)]
pub struct CommandFile {
    pub command: Vec<CommandEntry>,
}
