// PURPOSE: downloader-tui — surface: TUI surface (Ratatui)

pub mod surface_tui_state;
pub mod surface_tui_actions;
pub mod surface_tui_list;
pub mod surface_tui_draw;
pub mod surface_tui_event;
pub mod surface_tui_handler;

pub use surface_tui_handler::run;
