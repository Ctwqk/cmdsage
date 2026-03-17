pub mod keyword;
pub mod semantic;

use crate::knowledge::CommandEntry;

/// A matched command with its score
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub command: CommandEntry,
    pub score: f64,
    pub filled_template: Option<String>,
}
