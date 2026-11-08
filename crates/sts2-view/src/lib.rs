//! The presentation model shared by human clients.
//!
//! Nothing here knows a terminal, a file or a window: [`text`] turns the game's markup into
//! [`Role`](text::Role)-tagged [`Rich`](text::Rich) text, [`names`] gives engine values their
//! display names, [`describe`] fills the game's descriptions from the engine, and [`menu`] words
//! the out-of-combat choices. The TUI and the iOS app only draw the result.
//!
//! The rules live in `sts2-core`; this crate never decides what is legal or what a number is.

pub mod combat;
pub mod describe;
pub mod details;
pub mod last;
pub mod launch;
pub mod menu;
pub mod model;
pub mod names;
pub mod rooms;
pub mod sheets;
pub mod text;
pub mod topbar;
pub mod ui;
pub mod visits;
