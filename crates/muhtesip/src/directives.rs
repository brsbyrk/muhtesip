//! Inline suppression comments: `# muhtesip: ignore[<rule>, ...]`.
//!
//! The convention is the ecosystem's — zizmor's `# zizmor: ignore[...]`, whose regex is
//! `# <tool>: ignore\[(.+)\](?:\s+.*)?$` and whose names are comma-separated and trimmed, with the
//! comment allowed to sit anywhere in the finding's span. Matching that shape means one habit works
//! in both tools. Three differences, all forced by this linter's model:
//!
//! - **The text is scanned, not the document.** The YAML dependency skips comments, so there is no
//!   comment to read out of a parsed `Document`.
//! - **Coverage is by indentation, not by span.** A directive covers its own line and every
//!   following line more indented than it: the block it introduces. A trailing comment on `run: |`
//!   therefore covers the script, and one on a job's `build:` line covers the whole job. zizmor can
//!   afford a real span because it has one; muhtesip's findings carry a line.
//! - **A directive that cannot do anything is reported, not ignored.** This is where muhtesip departs
//!   from zizmor, which stays silent about a misspelled rule name, and from its own earlier
//!   behaviour. A directive that names a rule this build does not have — or that lost its brackets —
//!   *reads* as a suppression and suppresses nothing, so the author's red build looks like a bug
//!   rather than a comment. Deciding whether a name exists needs the registry, so this module only
//!   reports what the text says ([`Directives::named`], [`Directives::malformed`]) and the
//!   `directive` rule judges it against the ids the registry has.
//!
//! Everything here is a pure function of the text: no I/O, no registry.

/// The marker that introduces a directive, up to (but not including) the opening bracket.
///
/// The bracket is kept separate so that `# muhtesip: ignore` with no bracket is recognisable as a
/// directive that lost its argument, while a sentence that merely mentions the marker is left alone.
const MARKER: &str = "# muhtesip: ignore";

/// One directive: the rule ids it names, and the lines it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    /// The rule ids named inside the brackets.
    rules: Vec<String>,
    /// The line the directive itself sits on, 1-based — where a finding about it points.
    line: usize,
    /// The first line it covers, 1-based.
    first: usize,
    /// The last line it covers, 1-based.
    last: usize,
}

/// The directives a document declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Directives {
    /// One entry per usable directive found, in document order.
    spans: Vec<Span>,
    /// The 1-based lines of markers that are not usable directives: no bracket at all, no closing
    /// bracket, or nothing inside the brackets.
    malformed: Vec<usize>,
}

impl Directives {
    /// Scan a document's text for directives.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::directives::Directives;
    ///
    /// // On the `build:` line, so it covers that job's block (lines 2 to 4).
    /// let directives =
    ///     Directives::parse("jobs:\n  build: # muhtesip: ignore[missing-timeout]\n    runs-on: ubuntu-latest\n    steps: []\n");
    ///
    /// assert!(directives.ignores(2, "missing-timeout"));
    /// assert!(directives.ignores(4, "missing-timeout"), "the whole block");
    /// assert!(!directives.ignores(2, "unpinned-action"), "only the named rule");
    /// ```
    pub fn parse(text: &str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let mut spans = Vec::new();
        let mut malformed = Vec::new();

        for (index, line) in lines.iter().enumerate() {
            let Some(marker) = line.find(MARKER) else {
                continue;
            };
            let rest = &line[marker + MARKER.len()..];
            let line_number = index + 1;
            // The bracket follows the marker immediately: that is the documented spelling, and the
            // ecosystem's. A space before it is therefore not a directive — but it is unmistakably an
            // attempt at one, so it is reported rather than passed over in silence.
            let Some(body) = rest.strip_prefix('[') else {
                if rest.trim().is_empty() || rest.trim_start().starts_with('[') {
                    malformed.push(line_number);
                }
                continue;
            };
            // No closing bracket, or nothing usable inside it: a directive that cannot suppress.
            let Some(end) = body.find(']') else {
                malformed.push(line_number);
                continue;
            };
            let rules: Vec<String> = body[..end]
                .split(',')
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .collect();
            if rules.is_empty() {
                malformed.push(line_number);
                continue;
            }

            // The block this directive introduces: every following line that is blank or more
            // indented than the directive's own line. A sibling (or shallower) line ends it.
            let base = indentation(line);
            let first = index + 1;
            let mut last = first;
            for (offset, next) in lines.iter().enumerate().skip(index + 1) {
                if next.trim().is_empty() || indentation(next) > base {
                    last = offset + 1;
                } else {
                    break;
                }
            }

            spans.push(Span {
                rules,
                line: line_number,
                first,
                last,
            });
        }

        Self { spans, malformed }
    }

    /// Whether a directive suppresses `rule` on `line`.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::directives::Directives;
    ///
    /// let directives = Directives::parse("jobs:\n  build: # muhtesip: ignore[id, unpinned-action]\n    steps: []\n");
    ///
    /// assert!(directives.ignores(2, "id"));
    /// assert!(directives.ignores(2, "unpinned-action"));
    /// assert!(!directives.ignores(2, "missing-timeout"));
    /// assert!(!directives.ignores(1, "id"), "a shallower line is outside the block");
    /// ```
    pub fn ignores(&self, line: usize, rule: &str) -> bool {
        self.spans.iter().any(|span| {
            line >= span.first && line <= span.last && span.rules.iter().any(|named| named == rule)
        })
    }

    /// Every rule name a directive names, with the 1-based line the directive sits on, in document
    /// order.
    ///
    /// The names are reported exactly as written. Whether a name is a rule this build has is not a
    /// question about the text, so it is not answered here.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::directives::Directives;
    ///
    /// let directives = Directives::parse("jobs:\n  build: # muhtesip: ignore[a, b]\n");
    /// assert_eq!(directives.named().collect::<Vec<_>>(), vec![(2, "a"), (2, "b")]);
    /// ```
    pub fn named(&self) -> impl Iterator<Item = (usize, &str)> {
        self.spans.iter().flat_map(|span| {
            span.rules
                .iter()
                .map(move |rule| (span.line, rule.as_str()))
        })
    }

    /// The 1-based lines of markers that are not usable directives.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::directives::Directives;
    ///
    /// // A space before the bracket reads as a directive but is not one.
    /// let directives = Directives::parse("a: # muhtesip: ignore [id]\n");
    /// assert_eq!(directives.malformed().collect::<Vec<_>>(), vec![1]);
    /// ```
    pub fn malformed(&self) -> impl Iterator<Item = usize> {
        self.malformed.iter().copied()
    }

    /// Whether any directive was found at all — usable or not.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::directives::Directives;
    ///
    /// assert!(Directives::parse("jobs: {}\n").is_empty());
    /// assert!(!Directives::parse("a: # muhtesip: ignore[id]\n").is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty() && self.malformed.is_empty()
    }
}

/// The number of leading whitespace characters on a line.
fn indentation(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

#[cfg(test)]
mod tests {
    use super::Directives;

    #[test]
    fn a_trailing_comment_covers_the_block_it_introduces() {
        // zizmor's documented example: the comment sits on the `run: |` line and covers the script.
        let text = "\
jobs:
  build:
    steps:
      - run: | # muhtesip: ignore[deprecated-commands]
          echo '::set-output name=x::1'
      - run: echo hello
";
        let directives = Directives::parse(text);
        for line in 4..=5 {
            assert!(
                directives.ignores(line, "deprecated-commands"),
                "line {line}"
            );
        }
        assert!(
            !directives.ignores(6, "deprecated-commands"),
            "the span ends at the sibling step"
        );
    }

    #[test]
    fn a_directive_on_a_job_line_covers_the_whole_job() {
        let text = "\
jobs:
  build: # muhtesip: ignore[missing-timeout]
    runs-on: ubuntu-latest
    steps:
      - run: echo hello
  other:
    runs-on: ubuntu-latest
";
        let directives = Directives::parse(text);
        for line in 2..=5 {
            assert!(directives.ignores(line, "missing-timeout"), "line {line}");
        }
        assert!(
            !directives.ignores(6, "missing-timeout"),
            "the next job is out"
        );
    }

    #[test]
    fn a_blank_line_inside_the_block_does_not_end_the_span() {
        let text = "\
a: # muhtesip: ignore[x]
  b: 1

  c: 2
d: 3
";
        let directives = Directives::parse(text);
        assert!(
            directives.ignores(4, "x"),
            "the blank line does not end the block"
        );
        assert!(!directives.ignores(5, "x"), "a sibling ends it");
    }

    #[test]
    fn only_the_named_rules_are_ignored() {
        let text = "a: # muhtesip: ignore[one, two]\n  b: 1\n";
        let directives = Directives::parse(text);
        assert!(directives.ignores(1, "one"));
        assert!(directives.ignores(1, "two"));
        assert!(!directives.ignores(1, "three"));
        assert!(!directives.ignores(1, "on"), "a prefix is not a match");
    }

    #[test]
    fn trailing_explanation_after_the_brackets_is_allowed() {
        let text = "a: # muhtesip: ignore[one] i promise this is safe\n";
        assert!(Directives::parse(text).ignores(1, "one"));
    }

    #[test]
    fn a_malformed_directive_suppresses_nothing() {
        for text in [
            "a: # muhtesip: ignore[one\n",  // no closing bracket
            "a: # muhtesip: ignore[]\n",    // nothing named
            "a: # muhtesip: ignore[ , ]\n", // nothing but separators
            "a: # muhtesip: ignore one\n",  // no brackets at all
            "a: # Muhtesip: ignore[one]\n", // wrong case
        ] {
            let directives = Directives::parse(text);
            for line in 1..=2 {
                assert!(
                    !directives.ignores(line, "one"),
                    "must not suppress with {text:?}"
                );
            }
        }
    }

    #[test]
    fn a_marker_that_cannot_be_used_is_recorded_as_malformed() {
        // Four ways to write a directive that cannot suppress anything. They read as a suppression,
        // so they are ours to report rather than to pass over in silence.
        for text in [
            "a: # muhtesip: ignore[one\n",
            "a: # muhtesip: ignore[]\n",
            "a: # muhtesip: ignore[ , ]\n",
            "a: # muhtesip: ignore\n",
            "a: # muhtesip: ignore [one]\n", // a space before the bracket: an attempt, not a directive
        ] {
            let directives = Directives::parse(text);
            assert_eq!(
                directives.malformed().collect::<Vec<_>>(),
                vec![1],
                "{text:?}"
            );
            assert!(
                !directives.is_empty(),
                "a malformed directive is still a directive: {text:?}"
            );
        }
    }

    #[test]
    fn prose_that_mentions_the_marker_is_not_a_directive() {
        // A sentence, or a wrong-case marker: neither is an attempt at a directive. Reporting these
        // would be the false positive that gets a linter switched off, so they are left alone.
        for text in [
            "a: # muhtesip: ignore one\n",
            "a: # muhtesip: ignore this for now, see the ticket\n",
            "a: # Muhtesip: ignore[one]\n",
        ] {
            let directives = Directives::parse(text);
            assert!(directives.is_empty(), "must not report {text:?}");
            assert_eq!(directives.malformed().count(), 0, "{text:?}");
        }
    }

    #[test]
    fn named_directives_report_their_own_line_and_every_name() {
        let text = "jobs:\n  build: # muhtesip: ignore[one, two]\n    runs-on: ubuntu-latest\n";
        let directives = Directives::parse(text);
        assert_eq!(
            directives.named().collect::<Vec<_>>(),
            vec![(2, "one"), (2, "two")]
        );
    }

    #[test]
    fn a_document_without_directives_ignores_nothing() {
        let directives = Directives::parse("jobs:\n  build:\n    runs-on: ubuntu-latest\n");
        assert!(directives.is_empty());
        assert!(!directives.ignores(1, "missing-timeout"));
    }

    #[test]
    fn a_directive_inside_a_literal_block_does_not_reach_the_blocks_other_lines() {
        // YAML sees this comment as string content, and zizmor documents the same limitation. The
        // supported placement is the line that introduces the block.
        let text = "\
a: |
  line one
  # muhtesip: ignore[x]
  line three
b: 2
";
        let directives = Directives::parse(text);
        assert!(directives.ignores(3, "x"), "its own line");
        assert!(
            !directives.ignores(4, "x"),
            "the same indentation is a sibling, not a child — put the directive on the line that opens the block"
        );
        assert!(
            !directives.ignores(5, "x"),
            "and a shallower sibling ends it"
        );
    }
}
