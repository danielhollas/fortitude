mod ast;
pub mod check;
pub mod cli;
pub mod explain;
mod rules;
mod settings;
use annotate_snippets::{Level, Renderer, Snippet};
use anyhow::Context;
use ast::{named_descendants, parse};
use colored::{ColoredString, Colorize};
use lazy_regex::regex_captures;
use settings::Settings;
use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};

// Rule categories and identity codes
// ----------------------------------
// Helps to sort rules into logical categories, and acts as a unique tag with which
// users can switch rules on and off.

/// The category of each rule defines the sort of problem it intends to solve.
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Category {
    /// Failure to parse a file.
    Error,
    /// Violation of style conventions.
    Style,
    /// Misuse of types and kinds.
    Typing,
    /// Failure to use modules or use them appropriately.
    Modules,
    /// Best practices for setting floating point precision.
    Precision,
    /// Check path names, directory structures, etc.
    FileSystem,
}

impl Category {
    fn from(s: &str) -> anyhow::Result<Self> {
        match s {
            "E" => Ok(Self::Error),
            "S" => Ok(Self::Style),
            "T" => Ok(Self::Typing),
            "M" => Ok(Self::Modules),
            "P" => Ok(Self::Precision),
            "F" => Ok(Self::FileSystem),
            _ => {
                anyhow::bail!("{} is not a rule category.", s)
            }
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Error => "E",
            Self::Style => "S",
            Self::Typing => "T",
            Self::Modules => "M",
            Self::Precision => "P",
            Self::FileSystem => "F",
        };
        write!(f, "{}", s)
    }
}

/// The combination of a rule category and a unique identifying number.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Code {
    pub category: Category,
    pub number: usize,
}

impl Code {
    pub const fn new(category: Category, number: usize) -> Self {
        Self { category, number }
    }

    pub fn from(code_str: &str) -> anyhow::Result<Self> {
        let (_, category_str, number_str) = regex_captures!(r"^([A-Z]+)([0-9]{3})$", code_str)
            .context(format!("{} is not a valid error code.", code_str))?;
        let category = Category::from(category_str)?;
        let number = number_str.parse::<usize>()?;
        Ok(Code::new(category, number))
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{:03}", self.category, self.number)
    }
}

// Violation type
// --------------

/// The location within a file at which a violation was detected
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViolationPosition {
    None,
    Line(usize),
    LineCol((usize, usize)),
}

// This type is created when a rule is broken. As not all broken rules come with a
// line number or column, it is recommended to use the `violation!` macro to create
// these, or the `from_node()` function when creating them from `tree_sitter` queries.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Violation {
    /// Description of the error.
    message: String,
    /// The location at which the error was detected.
    position: ViolationPosition,
}

impl Violation {
    pub fn new<T: AsRef<str>>(message: T, position: ViolationPosition) -> Self {
        Self {
            message: String::from(message.as_ref()),
            position,
        }
    }

    pub fn from_node<T: AsRef<str>>(message: T, node: &tree_sitter::Node) -> Self {
        let position = node.start_position();
        Violation::new(
            message,
            ViolationPosition::LineCol((position.row + 1, position.column + 1)),
        )
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    pub fn position(&self) -> ViolationPosition {
        self.position
    }
}

#[macro_export]
macro_rules! violation {
    ($msg:expr) => {
        $crate::Violation::new($msg, $crate::ViolationPosition::None)
    };
    ($msg:expr, $line:expr) => {
        $crate::Violation::new($msg, $crate::ViolationPosition::Line($line))
    };
    ($msg:expr, $line:expr, $col:expr) => {
        $crate::Violation::new($msg, $crate::ViolationPosition::LineCol(($line, $col)))
    };
}

// Rule methods
// ------------

/// The methods by which rules are enforced. Some rules act on individual lines of code,
/// some by reading a full file, and others by analysing the concrete syntax tree. All
/// rules must be associated with a `Method` via the `Rule` trait.
pub enum Method {
    /// Methods that work on just the path name of the file.
    Path(fn(&Path) -> Option<Violation>),
    /// Methods that analyse the syntax tree.
    Tree(fn(&tree_sitter::Node, &str) -> Option<Violation>),
    /// Methods that analyse lines of code directly, using regex or otherwise.
    Text(fn(&str, &Settings) -> Vec<Violation>),
}

// Rule trait
// ----------

/// Should be implemented for all rules.
pub trait Rule {
    /// Return a function pointer to the method associated with this rule.
    fn method(&self) -> Method;

    /// Return text explaining what the rule tests for, why this is important, and how the user
    /// might fix it.
    fn explain(&self) -> &str;

    /// Return list of tree-sitter node types on which a rule should trigger.
    /// Path-based rules should return a vector containing only "PATH" while text-based rules
    /// should return a vector containing only "TEXT".
    fn entrypoints(&self) -> Vec<&str>;

    /// Apply a rule over some text, generating all violations raised as a result.
    fn apply(&self, source: &str, settings: &Settings) -> anyhow::Result<Vec<Violation>> {
        match self.method() {
            Method::Tree(f) => {
                let entrypoints = self.entrypoints();
                Ok(named_descendants(&parse(source)?.root_node())
                    .filter(|x| entrypoints.contains(&x.kind()))
                    .filter_map(|x| f(&x, source))
                    .collect())
            }
            Method::Text(f) => Ok(f(source, settings)),
            _ => {
                anyhow::bail!("Apply may only be called for tree and text methods.")
            }
        }
    }
}

// Violation diagnostics
// ---------------------

/// Reports of each violation. They are pretty-printable and sortable.
#[derive(Eq)]
pub struct Diagnostic {
    /// The file where an error was reported.
    path: PathBuf,
    /// The rule code that was violated, expressed as a string.
    code: String,
    /// The specific violation detected
    violation: Violation,
}

impl Diagnostic {
    pub fn new<P, S>(path: P, code: S, violation: &Violation) -> Self
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        Self {
            path: path.as_ref().to_path_buf(),
            code: code.as_ref().to_string(),
            violation: violation.clone(),
        }
    }

    fn orderable(&self) -> (&Path, usize, usize, &str) {
        match self.violation.position() {
            ViolationPosition::None => (self.path.as_path(), 0, 0, self.code.as_str()),
            ViolationPosition::Line(line) => (self.path.as_path(), line, 0, self.code.as_str()),
            ViolationPosition::LineCol((line, col)) => {
                (self.path.as_path(), line, col, self.code.as_str())
            }
        }
    }
}

impl Ord for Diagnostic {
    fn cmp(&self, other: &Self) -> Ordering {
        self.orderable().cmp(&other.orderable())
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        self.orderable() == other.orderable()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path = self.path.to_string_lossy().bold();
        let code = self.code.bold().bright_red();
        let message = self.violation.message();
        match self.violation.position() {
            ViolationPosition::None => {
                write!(f, "{}: {} {}", path, code, message)
            }
            ViolationPosition::Line(line) => {
                format_violation_line_col(self, f, line, 0, message, &path, &code)
            }
            ViolationPosition::LineCol((line, col)) => {
                format_violation_line_col(self, f, line, col, message, &path, &code)
            }
        }
    }
}

/// Read filename into vec of strings
fn read_lines(filename: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(filename)
        .unwrap() // panic on possible file-reading errors
        .lines() // split the string into an iterator of string slices
        .map(String::from) // make each slice into a string
        .collect() // gather them together into a vector
}

fn format_violation_line_col(
    diagnostic: &Diagnostic,
    f: &mut fmt::Formatter,
    line: usize,
    col: usize,
    message: &str,
    path: &ColoredString,
    code: &ColoredString,
) -> fmt::Result {
    let lines = read_lines(&diagnostic.path);
    let mut start_index = line.saturating_sub(2).max(1);

    // Trim leading empty lines.
    while start_index < line {
        if !lines[start_index.saturating_sub(1)].trim().is_empty() {
            break;
        }
        start_index = start_index.saturating_add(1);
    }

    let mut end_index = line.saturating_add(2).min(lines.len());

    // Trim leading empty lines.
    while end_index > line {
        if !lines[end_index.saturating_sub(1)].trim().is_empty() {
            break;
        }
        end_index = end_index.saturating_sub(1);
    }

    let content_slice = lines[start_index.saturating_sub(1)..end_index]
        .iter()
        .fold(String::default(), |acc, line| format!("{acc}{line}\n"));

    // Annotations are done by offset, so we need to count line
    // lengths... including the newline character, which doesn't
    // appear in `lines`!
    let offset_up_to_line = lines[start_index.saturating_sub(1)..line.saturating_sub(1)]
        .iter()
        .fold(0, |acc, line| acc + line.chars().count() + 1);

    // Something really weird going on here, where I can't get it to
    // put the annotation in the first column: it's either in column 2
    // or the end of the previous line. But does appear to be right
    // for other columns!
    let label_offset = offset_up_to_line + col.saturating_sub(1);

    // Some annoyance here: we *have* to have some level prefix to our
    // message. Might be fixed in future version of annotate-snippets
    // -- or we use an earlier version with more control.
    // Also, we could use `.origin(path)` to get the filename and
    // line:col automatically, but see above about off-by-one error
    let message_line = format!("{}:{}:{}: {} {}", path, line, col, code, message);
    let snippet = Level::Warning.title(&message_line).snippet(
        Snippet::source(&content_slice)
            .line_start(start_index)
            .annotation(
                Level::Error
                    .span(label_offset..label_offset.saturating_add(1))
                    .label(code),
            ),
    );

    let renderer = Renderer::styled();
    let source_block = renderer.render(snippet);
    writeln!(f, "{}", source_block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_code() {
        let f001 = Code::new(Category::FileSystem, 1);
        assert_eq!(f001.to_string(), "F001");
        let c120 = Code::new(Category::Style, 120);
        assert_eq!(c120.to_string(), "S120");
    }

    // TODO Test diagnostics
}
