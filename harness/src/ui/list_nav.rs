//! Harness stub: Step verbatim from zapfast src/ui/list_nav.rs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
    First,
    Last,
    PageUp(usize),
    PageDown(usize),
}
