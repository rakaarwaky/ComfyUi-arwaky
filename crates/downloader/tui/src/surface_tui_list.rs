// PURPOSE: downloader-tui — list filtering & selection methods

use downloader_file_utils::capabilities_file_checker::file_exists_valid;
use downloader_shared::taxonomy_model_vo::Model;

use crate::surface_tui_state::App;

impl App {
    pub fn mark_filtered_dirty(&mut self) {
        self.filtered_cache_dirty = true;
    }

    pub fn filtered_models(&mut self) -> Vec<(usize, Model)> {
        if self.filtered_cache_dirty {
            self.filtered_cache = self
                .models
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    let match_tab = match self.active_tab {
                        0 => true,
                        1 | 2 => {
                            let dest_dir = self.config.resolve_category_dir(&m.category);
                            let dest_path = dest_dir.join(&m.filename);
                            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));
                            if self.active_tab == 1 { exists } else { !exists }
                        }
                        _ => {
                            let cat_idx = self.active_tab - 3;
                            if cat_idx < self.categories.len() {
                                m.category.eq_ignore_ascii_case(&self.categories[cat_idx])
                            } else { true }
                        }
                    };
                    if !match_tab { return false; }
                    if self.search_query.is_empty() { true }
                    else {
                        let q = self.search_query.to_lowercase();
                        m.filename.to_lowercase().contains(&q)
                            || m.category.to_lowercase().contains(&q)
                            || m.group.to_lowercase().contains(&q)
                    }
                })
                .map(|(idx, m)| (idx, m.clone()))
                .collect();
            self.filtered_cache_dirty = false;
        }
        self.filtered_cache.clone()
    }

    pub fn toggle_selection(&mut self) {
        let filtered = self.filtered_models();
        if let Some(sel) = self.list_state.selected() {
            if sel < filtered.len() {
                let orig = filtered[sel].0;
                if let Some(pos) = self.selected_indices.iter().position(|&x| x == orig) {
                    self.selected_indices.remove(pos);
                    self.add_log(&format!("Deselected: {}", self.models[orig].filename));
                } else {
                    self.selected_indices.push(orig);
                    self.add_log(&format!("Selected: {}", self.models[orig].filename));
                }
            }
        }
    }

    pub fn select_group(&mut self, group: Option<&str>) {
        self.selected_indices.clear();
        for (i, m) in self.models.iter().enumerate() {
            let dest_dir = self.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            if !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                && group.as_ref().is_none_or(|g| m.group == *g) {
                    self.selected_indices.push(i);
                }
        }
        self.add_log(&format!("Bulk selected group '{}' ({} items).", group.unwrap_or("All Missing"), self.selected_indices.len()));
    }

    pub fn select_all_missing_in_view(&mut self) {
        let filtered = self.filtered_models();
        let mut count = 0;
        for (orig_idx, m) in filtered {
            let dest = self.config.resolve_category_dir(&m.category).join(&m.filename);
            if !file_exists_valid(&dest, m.size_bytes, Some(&m.url)) && !self.selected_indices.contains(&orig_idx) {
                self.selected_indices.push(orig_idx);
                count += 1;
            }
        }
        self.add_log(&format!("Selected {count} missing models in current view."));
    }

    pub fn ensure_active_tab_visible(&mut self, total_pages: usize) {
        if self.active_tab < self.tab_offset { self.tab_offset = self.active_tab; }
        else if self.active_tab >= self.tab_offset + 10 { self.tab_offset = self.active_tab.saturating_sub(9); }
        if self.tab_offset + 10 > total_pages { self.tab_offset = total_pages.saturating_sub(10); }
    }
}
