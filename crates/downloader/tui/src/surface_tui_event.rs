// PURPOSE: downloader-tui — surface: TUI keyboard/mouse event handling

use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::sync::atomic::Ordering;

use crate::surface_tui_state::{App, AppState, InputMode};

pub fn handle_event(
    app: &mut App,
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    event: Event,
) -> Result<bool, Box<dyn std::error::Error>> {
    match event {
        Event::Key(key) => {
            if key.kind == crossterm::event::KeyEventKind::Press {
                // Log viewer mode — takes priority over all states
                if app.log_viewer_visible {
                    match key.code {
                        KeyCode::Esc => {
                            app.log_viewer_visible = false;
                        }
                        KeyCode::Up => {
                            if app.log_scroll > 0 {
                                app.log_scroll -= 1;
                            }
                        }
                        KeyCode::Down => {
                            app.log_scroll += 1;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            let msg = app.copy_logs_to_clipboard();
                            app.log_viewer_visible = false;
                            app.add_log(&msg);
                        }
                        _ => {}
                    }
                    return Ok(true);
                }

                match app.state {
                    AppState::Menu => {
                        if app.input_mode == InputMode::Search {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    app.input_mode = InputMode::Normal;
                                }
                                KeyCode::Backspace => {
                                    app.search_query.pop();
                                    app.list_state.select(Some(0));
                                    app.mark_filtered_dirty();
                                }
                                KeyCode::Char(c) => {
                                    app.search_query.push(c);
                                    app.list_state.select(Some(0));
                                    app.mark_filtered_dirty();
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    if app.rx.is_some() {
                                        app.add_log("Cannot exit: download is in progress. Cancel first [x] or wait.");
                                    } else {
                                        return Ok(false);
                                    }
                                }
                                KeyCode::Char('c')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    if app.rx.is_some() {
                                        app.add_log("User cancelled downloading queue.");
                                        app.cancel_token.store(true, Ordering::Release);
                                    } else {
                                        return Ok(false);
                                    }
                                }
                                KeyCode::Char('x') | KeyCode::Char('X') => {
                                    if app.rx.is_some() {
                                        app.add_log("User cancelled downloading queue.");
                                        app.cancel_token.store(true, Ordering::Release);
                                    }
                                }
                                KeyCode::Char('/') => {
                                    app.input_mode = InputMode::Search;
                                }
                                KeyCode::Char('c') => {
                                    app.state = AppState::Settings {
                                        active_field: 0,
                                        models_dir_input: app.config.models_dir.clone(),
                                        hf_token_input: app
                                            .config
                                            .hf_token
                                            .clone()
                                            .unwrap_or_default(),
                                    };
                                    app.add_log("Settings menu opened.");
                                }
                                KeyCode::Tab | KeyCode::Right => {
                                    let total_pages = 3 + app.categories.len();
                                    app.active_tab = (app.active_tab + 1) % total_pages;
                                    app.ensure_active_tab_visible(total_pages);
                                    app.list_state.select(Some(0));
                                    app.mark_filtered_dirty();
                                }
                                KeyCode::BackTab | KeyCode::Left => {
                                    let total_pages = 3 + app.categories.len();
                                    app.active_tab = if app.active_tab == 0 {
                                        total_pages - 1
                                    } else {
                                        app.active_tab - 1
                                    };
                                    app.ensure_active_tab_visible(total_pages);
                                    app.list_state.select(Some(0));
                                    app.mark_filtered_dirty();
                                }
                                KeyCode::Up => {
                                    let filtered_len = app.filtered_models().len();
                                    if filtered_len > 0 {
                                        let i = match app.list_state.selected() {
                                            Some(i) => {
                                                if i == 0 {
                                                    filtered_len - 1
                                                } else {
                                                    i - 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.list_state.select(Some(i));
                                    }
                                }
                                KeyCode::Down => {
                                    let filtered_len = app.filtered_models().len();
                                    if filtered_len > 0 {
                                        let i = match app.list_state.selected() {
                                            Some(i) => {
                                                if i >= filtered_len - 1 {
                                                    0
                                                } else {
                                                    i + 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.list_state.select(Some(i));
                                    }
                                }
                                KeyCode::Char(' ') => {
                                    app.toggle_selection();
                                }
                                KeyCode::Char('a') | KeyCode::Char('A') => {
                                    app.select_all_missing_in_view();
                                }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    app.refresh_sizes();
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let digit =
                                        c.to_digit(10).expect("already checked is_ascii_digit")
                                            as usize;
                                    let total_pages = 3 + app.categories.len();
                                    if digit < total_pages {
                                        app.active_tab = digit;
                                        app.ensure_active_tab_visible(total_pages);
                                        app.list_state.select(Some(0));
                                        app.mark_filtered_dirty();
                                    }
                                }
                                KeyCode::Char('<') | KeyCode::Char(',') => {
                                    if app.tab_offset > 0 {
                                        app.tab_offset -= 1;
                                    }
                                }
                                KeyCode::Char('>') | KeyCode::Char('.') => {
                                    let total_pages = 3 + app.categories.len();
                                    if app.tab_offset + 10 < total_pages {
                                        app.tab_offset += 1;
                                    }
                                }
                                KeyCode::Char('d') | KeyCode::Enter => {
                                    app.check_space_and_start();
                                }
                                KeyCode::Char('l') | KeyCode::Char('L') => {
                                    app.log_viewer_visible = !app.log_viewer_visible;
                                    app.log_scroll = app.logs.len().saturating_sub(1);
                                }
                                _ => {}
                            }
                        }
                    }
                    AppState::Settings {
                        ref mut active_field,
                        ref mut models_dir_input,
                        ref mut hf_token_input,
                    } => match key.code {
                        KeyCode::Esc => {
                            app.state = AppState::Menu;
                            app.add_log("Settings menu closed without saving.");
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 4;
                        }
                        KeyCode::BackTab | KeyCode::Up => {
                            *active_field = if *active_field == 0 {
                                3
                            } else {
                                *active_field - 1
                            };
                        }
                        KeyCode::Enter => {
                            if *active_field == 2 {
                                // Save
                                app.config.models_dir = models_dir_input.clone();
                                app.config.hf_token = if hf_token_input.is_empty() {
                                    None
                                } else {
                                    Some(hf_token_input.clone())
                                };
                                if let Err(e) = app.save_config_to_file() {
                                    app.add_log(&format!("Failed to save config: {e:?}"));
                                } else {
                                    app.add_log("Configuration saved successfully.");
                                }
                                app.state = AppState::Menu;
                            } else if *active_field == 3 {
                                // Cancel
                                app.state = AppState::Menu;
                                app.add_log("Settings menu closed without saving.");
                            } else {
                                *active_field = (*active_field + 1) % 4;
                            }
                        }
                        KeyCode::Backspace => {
                            if *active_field == 0 {
                                models_dir_input.pop();
                            } else if *active_field == 1 {
                                hf_token_input.pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 {
                                models_dir_input.push(c);
                            } else if *active_field == 1 {
                                hf_token_input.push(c);
                            }
                        }
                        _ => {}
                    },
                    AppState::DiskSpaceWarning { .. } => match key.code {
                        KeyCode::Enter => {
                            app.add_log("Proceeding with download despite disk space warning.");
                            app.check_space_and_start();
                        }
                        KeyCode::Esc => {
                            app.state = AppState::Menu;
                        }
                        _ => {}
                    },
                    AppState::Finished { .. } => {
                        if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                            app.state = AppState::Menu;
                        }
                    }
                }
            }
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
            let size = terminal.size()?;

            let inner_rect = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(5),
                    Constraint::Length(7),
                    Constraint::Length(3),
                ])
                .split(size);

            let body_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(5)])
                .split(inner_rect[0]);

            let main_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(body_layout[1]);

            match app.state {
                AppState::Menu => {
                    // Click on Tab Navigation Bar
                    if mouse.row == body_layout[0].y {
                        let col = mouse.column.saturating_sub(body_layout[0].x);
                        if (2..7).contains(&col) {
                            if app.tab_offset > 0 {
                                app.tab_offset -= 1;
                            }
                        } else if (68..73).contains(&col) {
                            let total_pages = 3 + app.categories.len();
                            if app.tab_offset + 10 < total_pages {
                                app.tab_offset += 1;
                            }
                        } else if (8..68).contains(&col) {
                            let i = (col - 8) / 6;
                            let rem = (col - 8) % 6;
                            if rem < 5 {
                                let actual_idx = app.tab_offset + i as usize;
                                let total_pages = 3 + app.categories.len();
                                if actual_idx < total_pages {
                                    app.active_tab = actual_idx;
                                    app.list_state.select(Some(0));
                                }
                            }
                        }
                    }
                    // Click on Model List
                    else if mouse.row > main_layout[0].y
                        && mouse.row < main_layout[0].y + main_layout[0].height.saturating_sub(1)
                        && mouse.column > main_layout[0].x
                        && mouse.column < main_layout[0].x + main_layout[0].width.saturating_sub(1)
                    {
                        let clicked_row = mouse.row - (main_layout[0].y + 1);
                        let filtered = app.filtered_models();
                        let clicked_idx = app.list_state.offset() + clicked_row as usize;
                        if clicked_idx < filtered.len() {
                            if app.list_state.selected() == Some(clicked_idx) {
                                app.toggle_selection();
                            } else {
                                app.list_state.select(Some(clicked_idx));
                            }
                        }
                    }
                    // Click on Status Bar
                    else if mouse.row == inner_rect[2].y {
                        let col = mouse.column.saturating_sub(inner_rect[2].x);
                        let copy_start = if app.rx.is_some() { 115 } else { 130 };
                        if col >= copy_start && col <= copy_start + 15 {
                            let msg = app.copy_logs_to_clipboard();
                            app.add_log(&msg);
                        } else if app.input_mode == InputMode::Normal {
                            if (56..=74).contains(&col) {
                                app.input_mode = InputMode::Search;
                            } else if (75..=101).contains(&col) {
                                app.select_all_missing_in_view();
                            } else if (102..=117).contains(&col) {
                                app.refresh_sizes();
                            } else if (118..=134).contains(&col) {
                                app.state = AppState::Settings {
                                    active_field: 0,
                                    models_dir_input: app.config.models_dir.clone(),
                                    hf_token_input: app.config.hf_token.clone().unwrap_or_default(),
                                };
                            } else if (135..=154).contains(&col) {
                                app.check_space_and_start();
                            } else if col >= 155 {
                                if app.rx.is_some() {
                                    app.add_log("Cannot exit: download is in progress. Cancel first [x] or wait.");
                                } else {
                                    return Ok(false);
                                }
                            }
                        }
                    }

                    // Click Cancel in Sidebar Progress
                    if app.rx.is_some() {
                        let right_rects = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                            .split(main_layout[1]);
                        let cancel_row = right_rects[1].y + right_rects[1].height.saturating_sub(2);
                        if mouse.row == cancel_row
                            && mouse.column >= right_rects[1].x + 3
                            && mouse.column < right_rects[1].x + 26
                        {
                            app.add_log("User cancelled downloading queue.");
                            app.cancel_token.store(true, Ordering::Release);
                        }
                    }
                }
                AppState::Settings { .. } => { /* mouse handling unchanged from original */ }
                AppState::DiskSpaceWarning { .. } => {
                    let popup_rect = centered_rect(65, 30, size);
                    if mouse.row == popup_rect.y + 9 {
                        let col = mouse.column.saturating_sub(popup_rect.x);
                        if (4..=18).contains(&col) {
                            app.add_log("Proceeding with download despite disk space warning.");
                            app.check_space_and_start();
                        } else if (24..=37).contains(&col) {
                            app.state = AppState::Menu;
                        }
                    }
                }
                AppState::Finished { .. } => {
                    let popup_rect = centered_rect(50, 20, size);
                    if mouse.row == popup_rect.y + 6 {
                        let col = mouse.column.saturating_sub(popup_rect.x);
                        if (4..=13).contains(&col) {
                            app.state = AppState::Menu;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(true)
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
