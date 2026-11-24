//! Rendering the final plan.
//!
//! The one renderer projects a [`crate::state::Candidate`] into Markdown and
//! standalone HTML through a single presentation model; see [`plan`].

mod autocode;
mod diagram;
mod markdown;
pub mod plan;

pub use plan::{RenderContext, RenderedPlan, render};
