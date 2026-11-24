//! revy-review helper: validates the files a review writes (`problem.json`,
//! `reviews/<n>.json`, `investigations.json`, `verified.json`) and the accounting
//! across them, renders `report.md`, prints post-ready comments, and indexes the
//! rules and specialists. Parsing, checking and rendering are pure; `cli` and `git` are the only
//! modules that touch the filesystem or run processes.

pub mod check;
pub mod cli;
pub mod diff;
pub mod git;
pub mod investigations;
pub mod model;
pub mod problem;
pub mod report;
pub mod review;
pub mod rules;
pub mod specialists;
pub mod verified;
