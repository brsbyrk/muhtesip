//! Cron field syntax, and the interval the platform enforces on schedules.
//!
//! The platform documents the shape — "Use POSIX cron syntax to schedule workflows" — and names the
//! operators (`*`, `,`, `-`, `/`), but it does not publish the field bounds or say whether names like
//! `JAN` are allowed. Two decisions follow from that, both made to avoid the worse error:
//!
//! - **Names are accepted.** The incumbent validator accepts them, and rejecting a cron the platform
//!   accepts is a false positive, while accepting one it rejects is only a missing finding.
//! - **`@daily`-style descriptors are rejected**, because the docs state that outright: "GitHub Actions
//!   does not support the non-standard syntax `@yearly`, `@monthly`, `@weekly`, `@daily`, `@hourly`,
//!   and `@reboot`."
//!
//! Everything here is a pure function of the expression: no clock, no timezone database.

use crate::data::{CRON_FIELDS, CronField};

/// One parsed field: which of its values match.
struct Field {
    /// One entry per value the field may take: `true` when that value matches.
    allowed: Vec<bool>,
}

impl Field {
    /// Whether the field matches `value`.
    fn allows(&self, value: u32) -> bool {
        self.allowed.get(value as usize).copied().unwrap_or(false)
    }

    /// Whether the field matches no value at all.
    fn is_empty(&self) -> bool {
        self.allowed.iter().all(|allowed| !allowed)
    }
}

/// Why an expression is not a usable schedule, or `None` when it is one.
pub fn invalid(text: &str) -> Option<String> {
    parse(text).err()
}

/// The shortest gap between two firings, in minutes.
///
/// This is the number the platform's five-minute floor is about. It is computed over one day of
/// minutes, circularly, which is sufficient: a gap shorter than five minutes can only come from the
/// minute and hour fields, and the day fields can only ever make a gap **longer** by removing firings,
/// never shorter. `None` when the expression does not parse, or when no minute can ever match.
pub fn minimum_gap_minutes(text: &str) -> Option<u32> {
    let fields = parse(text).ok()?;
    let (minute, hour) = (&fields[0], &fields[1]);

    let mut matching: Vec<u32> = Vec::new();
    for minute_of_day in 0..1440u32 {
        if hour.allows(minute_of_day / 60) && minute.allows(minute_of_day % 60) {
            matching.push(minute_of_day);
        }
    }
    if matching.is_empty() {
        return None;
    }

    let mut smallest = u32::MAX;
    for (index, value) in matching.iter().enumerate() {
        let next = match matching.get(index + 1) {
            Some(next) => *next - *value,
            // The last firing of the day is followed by the first one of the next.
            None => matching[0] + 1440 - *value,
        };
        smallest = smallest.min(next);
    }
    Some(smallest)
}

/// Parse the five fields, or say which one is wrong and why.
fn parse(text: &str) -> Result<[Field; 5], String> {
    let trimmed = text.trim();
    if trimmed.starts_with('@') {
        // The docs name this list explicitly, so it is rejected as its own thing rather than as a
        // field that happens to have the wrong count.
        return Err(format!(
            "the platform does not support the non-standard syntax `{trimmed}`"
        ));
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() != CRON_FIELDS.len() {
        return Err(format!(
            "expected {} fields, found {}",
            CRON_FIELDS.len(),
            parts.len()
        ));
    }

    let parsed: Vec<Field> = parts
        .iter()
        .zip(CRON_FIELDS)
        .map(|(part, spec)| parse_field(part, spec))
        .collect::<Result<_, _>>()?;
    let out: [Field; 5] = parsed
        .try_into()
        .map_err(|_| "expected five fields".to_owned())?;

    // Only the calendar fields can make a date nothing matches, and a schedule that never fires is
    // worth saying even though its syntax is fine.
    if out[2].is_empty() || out[3].is_empty() {
        return Err("no day can match this expression, so it never fires".to_owned());
    }
    Ok(out)
}

/// Parse one field: a comma-separated list of terms.
fn parse_field(text: &str, spec: CronField) -> Result<Field, String> {
    let mut allowed = vec![false; (spec.max - spec.min + 1) as usize];
    for term in text.split(',') {
        apply_term(term.trim(), spec, &mut allowed)?;
    }
    Ok(Field { allowed })
}

/// Apply one term — `*`, `a`, `a-b`, `*/n`, `a-b/n` or `a/n` — to a field's allowed set.
fn apply_term(term: &str, spec: CronField, allowed: &mut [bool]) -> Result<(), String> {
    let (range, step) = match term.split_once('/') {
        Some((range, step)) => {
            let step: u32 = step
                .parse()
                .map_err(|_| format!("step '{step}' in {} is not a number", spec.name))?;
            if step == 0 {
                return Err(format!("the step in {} cannot be 0", spec.name));
            }
            (range, step)
        }
        None => (term, 1),
    };

    let (low, high) = if range == "*" {
        (spec.min, spec.max)
    } else if let Some((low, high)) = range.split_once('-') {
        (value_of(low, spec)?, value_of(high, spec)?)
    } else {
        let value = value_of(range, spec)?;
        // `a/n` runs from a to the end of the field, which is the common reading.
        if step == 1 {
            (value, value)
        } else {
            (value, spec.max)
        }
    };

    if low > high {
        return Err(format!("the {} range {low}-{high} is backwards", spec.name));
    }
    let mut value = low;
    while value <= high {
        allowed[(value - spec.min) as usize] = true;
        value += step;
    }
    Ok(())
}

/// One value of a field: a number, or a name where that field has names.
fn value_of(token: &str, spec: CronField) -> Result<u32, String> {
    if let Ok(number) = token.parse::<u32>() {
        if number < spec.min || number > spec.max {
            return Err(format!(
                "{number} is outside {}-{} for {}",
                spec.min, spec.max, spec.name
            ));
        }
        return Ok(number);
    }
    let lowered = token.to_ascii_lowercase();
    for (name, value) in spec.names {
        if *name == lowered {
            return Ok(*value);
        }
    }
    Err(format!(
        "'{token}' is not a number, a range, a step, or a {} name",
        spec.name
    ))
}

#[cfg(test)]
mod tests {
    use super::{invalid, minimum_gap_minutes};

    #[test]
    fn ordinary_schedules_are_valid() {
        for text in [
            "15 4,5 * * *", // the docs' own example
            "*/5 * * * *",
            "0 0 * * 1",
            "0-30/5 1 * * *",
            "5/15 * * * *",
            "0 0 1 JAN *", // names are accepted
            "30 22 * * fri",
            "0 0 * * 0",
        ] {
            assert_eq!(invalid(text), None, "{text}");
        }
    }

    #[test]
    fn a_descriptor_is_rejected_by_name() {
        for text in ["@daily", "@reboot", "@every 5m"] {
            let message = invalid(text).expect("a descriptor is not a schedule");
            assert!(message.contains("non-standard"), "{message}");
        }
    }

    #[test]
    fn the_field_count_is_checked() {
        let message = invalid("0 0 * *").expect("four fields");
        assert!(message.contains("expected 5 fields, found 4"), "{message}");
    }

    #[test]
    fn a_value_outside_its_field_is_named_with_its_bounds() {
        let message = invalid("75 * * * *").expect("minute 75");
        assert!(
            message.contains("75 is outside 0-59 for minute"),
            "{message}"
        );
        let message = invalid("0 0 * * 7").expect("POSIX numbers Sunday 0-6");
        assert!(message.contains("7 is outside 0-6"), "{message}");
    }

    #[test]
    fn malformed_terms_say_what_is_wrong() {
        let message = invalid("0 5-2 * * *").expect("backwards range");
        assert!(message.contains("backwards"), "{message}");
        let message = invalid("*/0 * * * *").expect("zero step");
        assert!(message.contains("cannot be 0"), "{message}");
        let message = invalid("x * * * *").expect("not a number");
        assert!(message.contains("not a number"), "{message}");
    }

    #[test]
    fn the_gap_is_the_shortest_time_between_two_firings() {
        for (text, expected) in [
            ("* * * * *", 1),
            ("*/2 * * * *", 2),
            ("*/4 * * * *", 4),
            ("*/5 * * * *", 5),
            ("0,5,10 * * * *", 5),
            ("59,0 * * * *", 1), // across the hour boundary, where a naive reading sees an hour
            ("0 * * * *", 60),
            ("15 4,5 * * *", 60), // the docs' example fires hourly
            ("0 0 * * *", 1440),
            ("0 0 1 * *", 1440), // the day field can only make the gap longer
        ] {
            assert_eq!(minimum_gap_minutes(text), Some(expected), "{text}");
        }
    }
}
