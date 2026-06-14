use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::utils::{file_exists_valid, format_size};
use super::app::{App, AppState, InputMode};

pub fn draw_ui(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.size();

    let title_suffix = if app.input_mode == InputMode::Search {
        format!(" [SEARCHING: {}] ", app.search_query)
    } else if !app.search_query.is_empty() {
        format!(" [Filter: {}] ", app.search_query)
    } else {
        String::new()
    };

    let outer_block = Block::default()
        .title(format!(" ComfyUI Desktop Model Downloader v2.2 (Ratatui TUI){} ", title_suffix))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(outer_block, size);

    let inner_rect = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(7), // Live Activity Log box
            Constraint::Length(3), // Status Bar Footer
        ])
        .split(size);

    // Filtered list
    let filtered = app.filtered_models();

    // Custom Paginated Tab Navigation Bar
    let total_pages = 3 + app.categories.len();
    let mut tab_spans = vec![
        Span::raw("  "),
        Span::styled("[ < ]", Style::default().fg(Color::Cyan)),
        Span::raw(" "),
    ];

    for i in 0..10 {
        let actual_idx = app.tab_offset + i;
        if actual_idx < total_pages {
            let label = if actual_idx == app.active_tab {
                if actual_idx < 10 {
                    format!(" [{}] ", actual_idx)
                } else {
                    format!(" [{}]", actual_idx)
                }
            } else {
                if actual_idx < 10 {
                    format!("  {}  ", actual_idx)
                } else {
                    format!("  {} ", actual_idx)
                }
            };

            let style = if actual_idx == app.active_tab {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Gray)
            };
            tab_spans.push(Span::styled(label, style));
        } else {
            tab_spans.push(Span::raw("      "));
        }
        tab_spans.push(Span::raw(" "));
    }

    tab_spans.push(Span::styled("[ > ]", Style::default().fg(Color::Cyan)));

    let active_name = match app.active_tab {
        0 => "All Models".to_string(),
        1 => "Installed Models".to_string(),
        2 => "Missing Models".to_string(),
        _ => {
            let cat_idx = app.active_tab - 3;
            if cat_idx < app.categories.len() {
                format!("Category: {}", app.categories[cat_idx])
            } else {
                "Unknown Category".to_string()
            }
        }
    };
    let active_label = format!("  Active Page: {} ({})", app.active_tab, active_name);

    let tab_paragraph = Paragraph::new(vec![
        Line::from(tab_spans),
        Line::from(Span::styled(active_label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
    ])
    .block(Block::default().borders(Borders::BOTTOM));

    let body_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(inner_rect[0]);

    f.render_widget(tab_paragraph, body_layout[0]);

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(body_layout[1]);

    // Draw Left Side: Model List
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, m)| {
            let dest_dir = app.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));

            let prefix = if app.selected_indices.contains(orig_idx) {
                "[✔] "
            } else {
                "[ ] "
            };

            let status_span = if exists {
                Span::styled(" READY ", Style::default().bg(Color::Green).fg(Color::Black))
            } else {
                Span::styled(" MISSING ", Style::default().bg(Color::Red).fg(Color::White))
            };

            let size = if m.size_bytes > 0 {
                m.size_bytes
            } else if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                *cache.sizes.get(&m.url).unwrap_or(&0)
            } else {
                0
            };

            let text = Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    format!("{:<45}", format!("{}/{}", m.category, m.filename)),
                    Style::default().fg(if exists { Color::DarkGray } else { Color::White }),
                ),
                Span::raw(format!(" {:>10}  ", format_size(size))),
                status_span,
            ]);

            ListItem::new(text)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Model List (Space to toggle) ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 41, 59))
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, main_layout[0], &mut app.list_state);

    // Draw Right Side: Model Info & Tips
    let right_rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[1]);

    let details_text = if let Some(selected) = app.list_state.selected() {
        if selected < filtered.len() {
            let m = &filtered[selected].1;
            let dest_dir = app.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));

            let size = if m.size_bytes > 0 {
                m.size_bytes
            } else if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                *cache.sizes.get(&m.url).unwrap_or(&0)
            } else {
                0
            };

            format!(
                "Filename: {}\nCategory: {}\nGroup: {}\nEstimated Size: {}\nStatus: {}\nNotes: {}\n\nURL: {}",
                m.filename,
                m.category,
                m.group,
                format_size(size),
                if exists { "✓ Installed" } else { "✗ Not Found" },
                m.notes,
                m.url
            )
        } else {
            "No model selected.".to_string()
        }
    } else {
        "No model selected.".to_string()
    };

    let details_paragraph = Paragraph::new(details_text)
        .block(Block::default().title(" Selected Model Info ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(details_paragraph, right_rects[0]);

    let guide_text = "RX6800XT 16GB VRAM Tips:\n\
                      - GGUF quants (Q5_K_S) are recommended for FLUX Dev.\n\
                      - FP8 quants are memory efficient.\n\
                      - Keep batch size to 1 for FLUX, max 2-3 for SDXL.\n\
                      - Set HSA_OVERRIDE_GFX_VERSION=10.3.0 in environment.";
    let guide_paragraph = Paragraph::new(guide_text)
        .block(Block::default().title(" GPU Optimization Guide ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(guide_paragraph, right_rects[1]);

    // Draw Bottom Logs
    let max_lines = 5;
    let log_start = app.logs.len().saturating_sub(max_lines);
    let logs_to_show = &app.logs[log_start..];
    let logs_text = logs_to_show.join("\n");
    let logs_paragraph = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title(" Live Activity Log ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true });
    f.render_widget(logs_paragraph, inner_rect[1]);

    // Draw Bottom Colored Status Bar Footer
    let help_text = if app.input_mode == InputMode::Search {
        "  [Type to Search]  |  [Enter/Esc] to exit search mode  |  [Backspace] to delete  "
    } else {
        "  [Click Tabs/Items]  |  [Tab/Shift+Tab] Cycle Tabs  |  [Space] Toggle  |  [a] Select All Missing  |  [c] Settings  |  [Enter/d] Download  "
    };
    let footer_paragraph = Paragraph::new(Span::styled(
        help_text,
        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
    ))
    .style(Style::default().bg(Color::Cyan));
    f.render_widget(footer_paragraph, inner_rect[2]);

    // Render Overlay Popups based on state
    match app.state {
        AppState::Menu => {}
        AppState::Settings {
            active_field,
            ref models_dir_input,
            ref hf_token_input,
        } => {
            let popup_rect = centered_rect(65, 45, size);
            f.render_widget(Clear, popup_rect);

            let active_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let inactive_style = Style::default().fg(Color::Gray);

            let dir_style = if active_field == 0 { active_style } else { inactive_style };
            let token_style = if active_field == 1 { active_style } else { inactive_style };
            let save_style = if active_field == 2 { active_style } else { inactive_style };
            let cancel_style = if active_field == 3 { active_style } else { inactive_style };

            let settings_spans = vec![
                Line::from(vec![
                    Span::styled("1. Models Download Directory:", dir_style),
                ]),
                Line::from(vec![
                    Span::styled(format!("   > {} ", models_dir_input), dir_style),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("2. HuggingFace Access Token (HF_TOKEN):", token_style),
                ]),
                Line::from(vec![
                    Span::styled(format!("   > {} ", hf_token_input), token_style),
                ]),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled("  [ SAVE ]  ", save_style),
                    Span::raw("      "),
                    Span::styled("  [ CANCEL ]  ", cancel_style),
                ]),
                Line::from(""),
                Line::from(Span::styled("Use [Tab] or [Up/Down] to navigate. Type to edit path/token. Save updates config.yaml.", Style::default().fg(Color::DarkGray))),
            ];

            let settings_paragraph = Paragraph::new(settings_spans)
                .block(
                    Block::default()
                        .title(" Settings & Configuration ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(settings_paragraph, popup_rect);
        }
        AppState::DiskSpaceWarning { required, available } => {
            let popup_rect = centered_rect(65, 30, size);
            f.render_widget(Clear, popup_rect);

            let warning_spans = vec![
                Line::from(Span::styled("⚠️ INSUFFICIENT DISK SPACE WARNING ⚠️", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(format!("Available Space: {}", format_size(available))),
                Line::from(format!("Total Required:  {}", format_size(required))),
                Line::from(""),
                Line::from("The download destination directory partition is running low."),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled("  [ PROCEED ]  ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw("      "),
                    Span::styled("  [ CANCEL ]  ", Style::default().fg(Color::Gray)),
                ]),
            ];

            let warning_paragraph = Paragraph::new(warning_spans)
                .block(
                    Block::default()
                        .title(" Disk Space Pre-Check ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(warning_paragraph, popup_rect);
        }
        AppState::Downloading {
            ref active_downloads,
            completed_count,
            failed_count,
            total_to_download,
        } => {
            let popup_rect = centered_rect(70, 42, size);
            f.render_widget(Clear, popup_rect);

            let mut progress_lines = vec![
                Line::from(format!(
                    "Overall Progress: Completed: {} | Failed: {} | Remaining Tasks: {}",
                    completed_count,
                    failed_count,
                    total_to_download.saturating_sub(completed_count + failed_count),
                )),
                Line::from(format!("Workers: {} Active", active_downloads.iter().filter(|x| x.is_some()).count())),
                Line::from(""),
            ];

            for (w_id, active) in active_downloads.iter().enumerate() {
                if let Some(dl) = active {
                    let pct = if dl.total_bytes > 0 {
                        (dl.bytes_downloaded as f64 / dl.total_bytes as f64 * 100.0) as u16
                    } else {
                        0
                    };
                    let bar_width = 25;
                    let bar = draw_progress_bar(pct, bar_width);
                    progress_lines.push(Line::from(format!("  Worker #{}: {} {}", w_id + 1, dl.filename, bar)));
                    progress_lines.push(Line::from(format!(
                        "    Progress: {}/{} | Speed: {:.2} MB/s | ETA: {}s",
                        format_size(dl.bytes_downloaded),
                        format_size(dl.total_bytes),
                        dl.speed_mb_s,
                        dl.eta_secs
                    )));
                } else {
                    progress_lines.push(Line::from(format!("  Worker #{}: Idle / Waiting for task...", w_id + 1)));
                    progress_lines.push(Line::from(""));
                }
                progress_lines.push(Line::from(""));
            }

            let used_lines = progress_lines.len() + 2; 
            let needed_padding = (popup_rect.height as usize).saturating_sub(used_lines + 2);
            for _ in 0..needed_padding {
                progress_lines.push(Line::from(""));
            }

            progress_lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled("  [ CANCEL DOWNLOAD ]  ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            ]));

            let progress_paragraph = Paragraph::new(progress_lines)
                .block(
                    Block::default()
                        .title(" Multi-Worker Download Queue ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                );
            f.render_widget(progress_paragraph, popup_rect);
        }
        AppState::Finished {
            completed,
            failed,
            ref message,
        } => {
            let popup_rect = centered_rect(50, 20, size);
            f.render_widget(Clear, popup_rect);

            let finished_spans = vec![
                Line::from(Span::styled(message.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(format!("Completed successfully: {}", completed)),
                Line::from(format!("Failed/Incomplete: {}", failed)),
                Line::from(""),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled("  [ OK ]  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]),
            ];

            let finished_paragraph = Paragraph::new(finished_spans)
                .block(
                    Block::default()
                        .title(" Task Finished ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(finished_paragraph, popup_rect);
        }
    }
}

fn draw_progress_bar(pct: u16, width: u16) -> String {
    let filled = ((pct as f32 / 100.0) * width as f32).round() as usize;
    let filled = std::cmp::min(filled, width as usize);
    let empty = (width as usize).saturating_sub(filled);
    format!(
        "[{}{}] {}%",
        "█".repeat(filled),
        "░".repeat(empty),
        pct
    )
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
