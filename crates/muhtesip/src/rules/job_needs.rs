//! The job-needs rule.

use std::collections::HashMap;

use crate::model::{Document, Job};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A job's dependencies must name defined jobs, must not repeat, and must not form a cycle.
///
/// A `needs` reference decides execution order, so a broken graph either never schedules the job
/// or schedules it before what it depends on.
pub struct JobNeeds;

impl JobNeeds {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "job-needs",
        severity: Severity::Error,
        description: "a `needs` reference is undefined, repeated, or part of a cycle",
    };
}

impl Rule for JobNeeds {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        // Job ids are case insensitive, so every comparison goes through the lower-cased key.
        let by_id: HashMap<String, usize> = doc
            .jobs
            .iter()
            .enumerate()
            .map(|(index, job)| (job.id.to_lowercase(), index))
            .collect();

        for job in &doc.jobs {
            let line = job.needs_line.unwrap_or(job.line);
            let mut seen: Vec<String> = Vec::new();
            for need in &job.needs {
                if need.contains(EXPRESSION_OPEN) {
                    continue;
                }
                let key = need.to_lowercase();
                if seen.contains(&key) {
                    findings.push(Finding {
                        rule: self.id(),
                        severity: self.meta().severity,
                        message: format!("job '{}' needs '{}' more than once", job.id, need),
                        line,
                    });
                    continue;
                }
                seen.push(key.clone());
                if !by_id.contains_key(&key) {
                    findings.push(Finding {
                        rule: self.id(),
                        severity: self.meta().severity,
                        message: format!(
                            "job '{}' needs '{}', which this workflow does not define",
                            job.id, need
                        ),
                        line,
                    });
                }
            }
        }

        for index in cycle_members(&doc.jobs, &by_id) {
            let job = &doc.jobs[index];
            findings.push(Finding {
                rule: self.id(),
                severity: self.meta().severity,
                message: format!("job '{}' is part of a dependency cycle", job.id),
                line: job.needs_line.unwrap_or(job.line),
            });
        }
    }
}

/// The indices of every job that takes part in a dependency cycle.
///
/// A depth-first walk marks the jobs on the current path; an edge back to a marked job proves a
/// cycle, and every job on the path from that one onward is in it. Reporting the *members* matters:
/// a job that merely depends on a cycle is not itself broken, and a leftover-node heuristic
/// (Kahn's algorithm) would wrongly accuse it.
fn cycle_members(jobs: &[Job], by_id: &HashMap<String, usize>) -> Vec<usize> {
    /// The job has not been reached yet.
    const UNSEEN: u8 = 0;
    /// The job is on the path currently being walked.
    const ON_PATH: u8 = 1;
    /// The job and everything it reaches have been walked.
    const DONE: u8 = 2;

    #[allow(clippy::too_many_arguments)]
    fn walk(
        node: usize,
        jobs: &[Job],
        by_id: &HashMap<String, usize>,
        marks: &mut [u8],
        path: &mut Vec<usize>,
        members: &mut [bool],
    ) {
        marks[node] = ON_PATH;
        path.push(node);
        for need in &jobs[node].needs {
            let Some(&target) = by_id.get(&need.to_lowercase()) else {
                continue;
            };
            match marks[target] {
                ON_PATH => {
                    if let Some(start) = path.iter().position(|&on_path| on_path == target) {
                        for &member in &path[start..] {
                            members[member] = true;
                        }
                    }
                }
                UNSEEN => walk(target, jobs, by_id, marks, path, members),
                _ => {}
            }
        }
        path.pop();
        marks[node] = DONE;
    }

    let mut marks = vec![UNSEEN; jobs.len()];
    let mut members = vec![false; jobs.len()];
    let mut path = Vec::new();
    let mut index = 0;
    while index < jobs.len() {
        if marks[index] == UNSEEN {
            walk(index, jobs, by_id, &mut marks, &mut path, &mut members);
        }
        index += 1;
    }
    (0..jobs.len()).filter(|&index| members[index]).collect()
}
