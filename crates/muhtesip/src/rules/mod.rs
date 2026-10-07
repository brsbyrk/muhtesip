//! The shipped rules. One module per rule: a zero-sized struct, its declarative metadata, and
//! its private helpers. No rule knows about any other; the registry composes them.

mod action_ref;
mod condition;
mod credentials;
mod deprecated_commands;
mod directive;
mod env_var;
mod events;
mod glob;
mod identifiers;
mod job_needs;
mod matrix;
mod missing_timeout;
mod permissions;
mod runner_label;
mod shell_name;
mod unpinned_action;

pub use action_ref::ActionRef;
pub use condition::Condition;
pub use credentials::Credentials;
pub use deprecated_commands::DeprecatedCommands;
pub use directive::Directive;
pub use env_var::EnvVarNames;
pub use events::TriggerEvents;
pub use glob::FilterPatterns;
pub use identifiers::Identifiers;
pub use job_needs::JobNeeds;
pub use matrix::MatrixCombinations;
pub use missing_timeout::MissingTimeout;
pub use permissions::Permissions;
pub use runner_label::RunnerLabel;
pub use shell_name::ShellName;
pub use unpinned_action::UnpinnedAction;

/// The opening of an expression in the document format.
pub(crate) const EXPRESSION_OPEN: &str = "${{";
/// The closing of an expression in the document format.
const EXPRESSION_CLOSE: &str = "}}";
