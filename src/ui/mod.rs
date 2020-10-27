mod graphview;
pub use graphview::*;

mod screenwrapper;
pub use screenwrapper::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum UiAction {
    Redraw,
    Close,
}
