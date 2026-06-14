use crossterm::event::{Event, KeyCode, KeyModifiers, MouseEventKind, MouseButton};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::sync::atomic::Ordering;

use super::app::{App, AppState, InputMode};

pub fn handle_event(
    app: &mut App,
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    event: Event,
) -> Result<bool, Box<dyn std::error::Error>> {
    match event {
        Event::Key(key) => {
            if key.kind == crossterm::event::KeyEventKind::Press {
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
                                }
                                KeyCode::Char(c) => {
                                    app.search_query.push(c);
                                    app.list_state.select(Some(0));
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                                KeyCode::Char('/') => {
                                    app.input_mode = InputMode::Search;
                                }
                                KeyCode::Char('c') => {
                                    app.state = AppState::Settings {
                                        active_field: 0,
                                        models_dir_input: app.config.models_dir.clone(),
                                        hf_token_input: app.config.hf_token.clone().unwrap_or_default(),
                                    };
                                    app.add_log("Settings menu opened.");
                                }
                                KeyCode::Tab | KeyCode::Right => {
                                    let total_pages = 3 + app.categories.len();
                                    app.active_tab = (app.active_tab + 1) % total_pages;
                                    app.ensure_active_tab_visible(total_pages);
                                    app.list_state.select(Some(0));
                                }
                                KeyCode::BackTab | KeyCode::Left => {
                                    let total_pages = 3 + app.categories.len();
                                    app.active_tab = if app.active_tab == 0 { total_pages - 1 } else { app.active_tab - 1 };
                                    app.ensure_active_tab_visible(total_pages);
                                    app.list_state.select(Some(0));
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
                                    app.refresh_selected_or_all_model_sizes();
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let digit = c.to_digit(10).unwrap() as usize;
                                    let total_pages = 3 + app.categories.len();
                                    if digit < total_pages {
                                        app.active_tab = digit;
                                        app.ensure_active_tab_visible(total_pages);
                                        app.list_state.select(Some(0));
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
                            *active_field = if *active_field == 0 { 3 } else { *active_field - 1 };
                        }
                        KeyCode::Enter => {
                            if *active_field == 2 {
                                // Save settings
                                app.config.models_dir = models_dir_input.clone();
                                app.config.hf_token = if hf_token_input.is_empty() {
                                    None
                                } else {
                                    Some(hf_token_input.clone())
                                };
                                if let Err(e) = app.save_config_to_file() {
                                    app.add_log(&format!("Failed to save config: {:?}", e));
                                } else {
                                    app.add_log("Configuration saved successfully.");
                                }
                                app.state = AppState::Menu;
                            } else if *active_field == 3 {
                                // Cancel settings
                                app.state = AppState::Menu;
                                app.add_log("Settings menu closed without saving.");
                            } else {
                                // Enter acts as Tab inside text fields
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
                            app.start_download();
                        }
                        KeyCode::Esc => {
                            app.state = AppState::Menu;
                        }
                        _ => {}
                    },
                    AppState::Downloading { .. } => {
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            app.add_log("User cancelled downloading queue.");
                            app.cancel_token.store(true, Ordering::Release);
                        }
                    }
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
                
                // Compute layout regions to map clicks
                let inner_rect = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Min(5),
                        Constraint::Length(7), // Live Activity Log box
                        Constraint::Length(3), // Status Bar Footer
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
                        // 1. Click on Tab Navigation Bar
                        if mouse.row == body_layout[0].y {
                            let col = mouse.column.saturating_sub(body_layout[0].x);
                            if (2..7).contains(&col) {
                                // Prev button [<]
                                if app.tab_offset > 0 {
                                    app.tab_offset -= 1;
                                    app.add_log("Shifted tabs left.");
                                }
                            } else if (68..73).contains(&col) {
                                // Next button [>]
                                let total_pages = 3 + app.categories.len();
                                if app.tab_offset + 10 < total_pages {
                                    app.tab_offset += 1;
                                    app.add_log("Shifted tabs right.");
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
                                        app.add_log(&format!("Switched to tab {}.", actual_idx));
                                    }
                                }
                            }
                        }
                        // 2. Click on Model List items
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
                        // 3. Click on Status Bar Footer
                        else if mouse.row == inner_rect[2].y {
                            let col = mouse.column.saturating_sub(inner_rect[2].x);
                            if app.input_mode == InputMode::Normal {
                                if (56..=74).contains(&col) {
                                    app.input_mode = InputMode::Search;
                                } else if (75..=101).contains(&col) {
                                    app.select_all_missing_in_view();
                                } else if (102..=117).contains(&col) {
                                    app.refresh_selected_or_all_model_sizes();
                                } else if (118..=134).contains(&col) {
                                    app.state = AppState::Settings {
                                        active_field: 0,
                                        models_dir_input: app.config.models_dir.clone(),
                                        hf_token_input: app.config.hf_token.clone().unwrap_or_default(),
                                    };
                                    app.add_log("Settings menu opened.");
                                } else if (135..=154).contains(&col) {
                                    app.check_space_and_start();
                                } else if col >= 155 {
                                    return Ok(false); // Exit downloader
                                }
                            }
                        }
                    }
                    AppState::Settings {
                        ref mut active_field,
                        ref mut models_dir_input,
                        ref mut hf_token_input,
                    } => {
                        let popup_rect = centered_rect(65, 45, size);
                        if mouse.row == popup_rect.y + 1 || mouse.row == popup_rect.y + 2 {
                            *active_field = 0;
                        } else if mouse.row == popup_rect.y + 4 || mouse.row == popup_rect.y + 5 {
                            *active_field = 1;
                        } else if mouse.row == popup_rect.y + 8 {
                            let col = mouse.column.saturating_sub(popup_rect.x);
                            if (4..=15).contains(&col) {
                                // Save
                                app.config.models_dir = models_dir_input.clone();
                                app.config.hf_token = if hf_token_input.is_empty() {
                                    None
                                } else {
                                    Some(hf_token_input.clone())
                                };
                                if let Err(e) = app.save_config_to_file() {
                                    app.add_log(&format!("Failed to save config: {:?}", e));
                                } else {
                                    app.add_log("Configuration saved successfully.");
                                }
                                app.state = AppState::Menu;
                            } else if (22..=35).contains(&col) {
                                // Cancel
                                app.state = AppState::Menu;
                                app.add_log("Settings menu closed without saving.");
                            }
                        }
                    }
                    AppState::DiskSpaceWarning { .. } => {
                        let popup_rect = centered_rect(65, 30, size);
                        if mouse.row == popup_rect.y + 9 {
                            let col = mouse.column.saturating_sub(popup_rect.x);
                            if (4..=18).contains(&col) {
                                // Proceed
                                app.add_log("Proceeding with download despite disk space warning.");
                                app.start_download();
                            } else if (24..=37).contains(&col) {
                                // Cancel
                                app.state = AppState::Menu;
                            }
                        }
                    }
                    AppState::Downloading { .. } => {
                        let popup_rect = centered_rect(70, 42, size);
                        if mouse.row == popup_rect.y + popup_rect.height.saturating_sub(2) {
                            let col = mouse.column.saturating_sub(popup_rect.x);
                            if (4..=27).contains(&col) {
                                // Cancel download
                                app.add_log("User cancelled downloading queue.");
                                app.cancel_token.store(true, Ordering::Release);
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
        }
        _ => {}
    }
    Ok(true)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
