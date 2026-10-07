//! Validate the filter patterns the platform accepts in `branches`, `tags`, and `paths`.
//!
//! Adapted from another linter's `globValidator` state machine. Two deliberate differences:
//!
//! - **No columns.** A finding points at the pattern's line; the model here carries lines and not
//!   columns. The detections are the same.
//! - **No whitespace skipping while peeking.** Every character is read. GitHub does not accept
//!   whitespace in a ref name anyway, and the path check already reports leading and trailing
//!   spaces, so the practical difference is nil — but it is a difference.
//!
//! Syntax reference: the platform's "filter pattern cheat sheet".

/// Read one character at a time, reporting what is wrong with each.
struct Validator {
    /// The pattern, as characters, so a byte is never split.
    characters: Vec<char>,
    /// How far into `characters` the scan has reached.
    position: usize,
    /// Whether the pattern describes a git ref (branch or tag) rather than a file path.
    is_ref: bool,
    /// Whether the previous character was ordinary, so `?` or `+` may follow it.
    preceded_by_ordinary: bool,
    /// The problems found so far; empty means the pattern is usable.
    errors: Vec<String>,
}

/// The sentinel returned once the pattern is exhausted.
///
/// A NUL cannot appear in a workflow pattern, so it cannot be confused with real input.
const END: char = '\0';

/// The problems with a pattern used for branch or tag names.
pub(crate) fn ref_pattern_errors(pattern: &str) -> Vec<String> {
    validate(pattern, true)
}

/// The problems with a pattern used for file paths.
pub(crate) fn path_pattern_errors(pattern: &str) -> Vec<String> {
    let mut errors = Vec::new();

    let trimmed = pattern.trim();
    if pattern != trimmed {
        errors.push("leading and trailing spaces are not allowed in a path pattern".to_owned());
    }

    // `.` and `..` are not handled by the path filter.
    let body = trimmed.strip_prefix('!').unwrap_or(trimmed);
    if body == "." || body == ".." || body.starts_with("./") || body.starts_with("../") {
        errors.push("'.' and '..' are not allowed in a path pattern".to_owned());
    }

    if !errors.is_empty() {
        return errors;
    }

    validate(pattern, false)
}

/// Run the state machine over a pattern.
fn validate(pattern: &str, is_ref: bool) -> Vec<String> {
    let mut validator = Validator {
        characters: pattern.chars().collect(),
        position: 0,
        is_ref,
        preceded_by_ordinary: false,
        errors: Vec::new(),
    };
    validator.run();
    validator.errors
}

impl Validator {
    /// The character at the cursor, or `END` once the pattern is exhausted.
    fn peek(&self) -> char {
        self.characters.get(self.position).copied().unwrap_or(END)
    }

    /// Consume and return the character at the cursor, advancing past it.
    fn take(&mut self) -> char {
        let character = self.peek();
        if character != END {
            self.position += 1;
        }
        character
    }

    /// Record one problem with the pattern.
    fn error(&mut self, message: String) {
        self.errors.push(message);
    }

    /// Report a character where something else was expected.
    fn unexpected(&mut self, found: char, what: &str, why: &str) {
        let found = if found == END {
            "the pattern ends unexpectedly".to_owned()
        } else {
            format!("unexpected character '{found}'")
        };
        let checking = if what.is_empty() {
            String::new()
        } else {
            format!(" while checking {what}")
        };
        self.error(format!("invalid filter pattern: {found}{checking}. {why}"));
    }

    /// Report a character that a git ref name cannot contain.
    fn invalid_ref_char(&mut self, character: char, why: &str) {
        self.error(format!(
            "character '{character}' is invalid in a branch or tag name: {why}"
        ));
    }

    /// Walk the whole pattern, recording every problem found.
    fn run(&mut self) {
        if self.characters.is_empty() {
            self.error("the filter pattern is empty".to_owned());
            return;
        }

        // The first character needs its own handling for `/` and a leading `!`.
        match self.peek() {
            '/' => {
                if self.is_ref {
                    self.take();
                    self.invalid_ref_char('/', "a ref name must not start with '/'");
                    self.preceded_by_ordinary = true;
                }
            }
            '!' => {
                self.take();
                if self.peek() == END {
                    self.unexpected(
                        '!',
                        "a leading '!' that negates the pattern",
                        "at least one character must follow '!'",
                    );
                    return;
                }
                self.preceded_by_ordinary = false;
            }
            _ => {}
        }

        while self.step() {}
    }

    /// Read one character. Returns `false` once the pattern is exhausted.
    fn step(&mut self) -> bool {
        let mut character = self.take();
        let mut ordinary = true;

        match character {
            '\\' => match self.peek() {
                '[' | '?' | '*' => {
                    character = self.take();
                    if self.is_ref {
                        let next = self.peek();
                        self.invalid_ref_char(next, REF_FORBIDDEN);
                    }
                }
                '+' | '\\' | '!' => character = self.take(),
                _ => {
                    // A file path may contain a backslash (`mkdir 'foo\bar'` works).
                    if self.is_ref {
                        self.invalid_ref_char(
                            '\\',
                            "only the special characters [, ?, +, *, \\, and ! may be escaped",
                        );
                        character = self.take();
                    }
                }
            },
            '?' => {
                if !self.preceded_by_ordinary {
                    self.unexpected(
                        '?',
                        "the special character '?' (zero or one)",
                        "the preceding character must not be a special character",
                    );
                }
                ordinary = false;
            }
            '+' => {
                if !self.preceded_by_ordinary {
                    self.unexpected(
                        '+',
                        "the special character '+' (one or more)",
                        "the preceding character must not be a special character",
                    );
                }
                ordinary = false;
            }
            '*' => ordinary = false,
            '[' => self.character_class(),
            '\r' => {
                if self.peek() == '\n' {
                    character = self.take();
                }
                self.unexpected(character, "", "a newline cannot appear in a pattern");
            }
            '\n' => self.unexpected('\n', "", "a newline cannot appear in a pattern"),
            ' ' | '\t' | '~' | '^' | ':' if self.is_ref => {
                self.invalid_ref_char(character, REF_FORBIDDEN);
            }
            ' ' | '\t' | '~' | '^' | ':' => {}
            _ => {}
        }

        self.preceded_by_ordinary = ordinary;

        if self.peek() == END {
            if self.is_ref && (character == '/' || character == '.') {
                self.invalid_ref_char(character, "a ref name must not end with '/' or '.'");
            }
            return false;
        }
        true
    }

    /// Read a `[...]` character class and report what is wrong with it.
    fn character_class(&mut self) {
        if self.peek() == ']' {
            let closing = self.take();
            self.unexpected(
                closing,
                "the content of a character class []",
                "a character class must not be empty",
            );
            return;
        }

        let mut characters = 0_usize;
        loop {
            let current = self.take();
            match current {
                ']' if characters == 1 => {
                    self.unexpected(
                        current,
                        "a character class []",
                        "a class holding one character is useless; write the character itself",
                    );
                    return;
                }
                ']' => return,
                END => {
                    self.unexpected(
                        current,
                        "the end of a character class []",
                        "a ']' is missing",
                    );
                    return;
                }
                _ => {
                    if self.peek() != '-' {
                        // A single character.
                        characters += 1;
                        continue;
                    }

                    // A range such as `0-9`.
                    characters += 2;
                    let start = current;
                    self.take(); // the '-'
                    match self.peek() {
                        ']' => {
                            let closing = self.take();
                            self.unexpected(
                                closing,
                                "a character range in []",
                                "the end of the range is missing",
                            );
                            return;
                        }
                        END => {}
                        _ => {
                            let end = self.take();
                            if start > end {
                                let why = format!(
                                    "the range starts at '{start}' and ends at '{end}', which is backwards"
                                );
                                self.unexpected(end, "a character range in []", &why);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What a git ref name may not contain.
const REF_FORBIDDEN: &str = "ref names cannot contain spaces, ~, ^, :, [, ?, or *";

#[cfg(test)]
mod tests {
    use super::{path_pattern_errors, ref_pattern_errors};

    #[test]
    fn ordinary_ref_patterns_are_accepted() {
        for pattern in [
            "main",
            "releases/**",
            "v1.*",
            "v1.+",
            "**",
            "feature/*",
            "!main",
            "[a-z]+",
            "**/next",
        ] {
            assert!(
                ref_pattern_errors(pattern).is_empty(),
                "{pattern}: {:?}",
                ref_pattern_errors(pattern)
            );
        }
    }

    #[test]
    fn a_ref_name_rejects_the_characters_git_rejects() {
        for pattern in ["foo bar", "foo~1", "foo^", "foo:bar", "rel/1.0/"] {
            assert!(
                !ref_pattern_errors(pattern).is_empty(),
                "{pattern} must be rejected"
            );
        }
    }

    #[test]
    fn a_ref_name_may_not_lead_or_trail_with_a_separator() {
        assert!(!ref_pattern_errors("/main").is_empty());
        assert!(!ref_pattern_errors("main/").is_empty());
        assert!(!ref_pattern_errors("main.").is_empty());
    }

    #[test]
    fn a_repetition_must_follow_an_ordinary_character() {
        assert!(ref_pattern_errors("v1.+").is_empty(), "after '1' is fine");
        assert!(
            !ref_pattern_errors("?main").is_empty(),
            "nothing precedes it"
        );
        assert!(!ref_pattern_errors("+main").is_empty());
        assert!(!ref_pattern_errors("*?main").is_empty(), "'*' precedes it");
    }

    #[test]
    fn a_character_class_is_checked() {
        assert!(ref_pattern_errors("[0-9]").is_empty());
        assert!(!ref_pattern_errors("[abc").is_empty(), "no closing bracket");
        assert!(!ref_pattern_errors("[]").is_empty(), "empty class");
        assert!(
            !ref_pattern_errors("[a]").is_empty(),
            "a class of one is useless"
        );
        assert!(!ref_pattern_errors("[z-a]").is_empty(), "backwards range");
        assert!(!ref_pattern_errors("[0-]").is_empty(), "missing range end");
    }

    #[test]
    fn an_empty_pattern_is_reported() {
        assert!(!ref_pattern_errors("").is_empty());
        assert!(!path_pattern_errors("").is_empty());
    }

    #[test]
    fn a_newline_is_reported() {
        assert!(!ref_pattern_errors("main\n").is_empty());
    }

    #[test]
    fn path_patterns_reject_leading_spaces_and_dots() {
        assert!(path_pattern_errors("src/**").is_empty());
        assert!(path_pattern_errors("**/*.js").is_empty());
        assert!(!path_pattern_errors(" src/x").is_empty(), "leading space");
        assert!(!path_pattern_errors("src/x ").is_empty(), "trailing space");
        assert!(!path_pattern_errors(".").is_empty());
        assert!(!path_pattern_errors("../x").is_empty());
        assert!(!path_pattern_errors("./x").is_empty());
    }

    #[test]
    fn a_path_may_hold_what_a_ref_may_not() {
        // A path may contain a space and a backslash; a ref may not.
        assert!(path_pattern_errors("my dir/**").is_empty());
        assert!(path_pattern_errors(r"dir\file").is_empty());
        assert!(!ref_pattern_errors("my dir/**").is_empty());
    }
}
