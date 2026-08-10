//! analyze TUI 状态机（Space / Filter / Top / Delete 确认等纯逻辑）。

use std::collections::BTreeSet;

use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeKey {
    Up,
    Down,
    Enter,
    Esc,
    Quit,
    Space,
    Delete,
    Open,
    Preview,
    Filter,
    Top,
    FilterChar(char),
    FilterBackspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeEffect {
    None,
    EnterDir(String),
    GoBack,
    Quit,
    Open(Vec<String>),
    Preview(String),
    RequestDelete(Vec<String>),
    ConfirmDelete,
    CancelDelete,
}

#[derive(Debug, Clone, Default)]
pub struct AnalyzeState {
    pub selected: usize,
    pub show_large_files: bool,
    pub multi_selected: BTreeSet<String>,
    pub large_multi_selected: BTreeSet<String>,
    pub entry_filter: String,
    pub large_filter: String,
    pub entry_filtering: bool,
    pub large_filtering: bool,
    pub delete_confirm: bool,
    pub status: String,
}

impl AnalyzeState {
    pub fn visible_entries<'a>(&self, out: &'a AnalyzeOutput) -> Vec<&'a AnalyzeEntry> {
        let q = self.entry_filter.to_lowercase();
        out.entries
            .iter()
            .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn visible_large<'a>(&self, out: &'a AnalyzeOutput) -> Vec<&'a AnalyzeFileEntry> {
        let q = self.large_filter.to_lowercase();
        out.large_files
            .iter()
            .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn clamp_selection(&mut self, out: &AnalyzeOutput) {
        let len = if self.show_large_files {
            self.visible_large(out).len()
        } else {
            self.visible_entries(out).len()
        };
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    pub fn handle_key(
        &mut self,
        key: AnalyzeKey,
        out: &AnalyzeOutput,
        scanning: bool,
        can_go_back: bool,
    ) -> AnalyzeEffect {
        if self.delete_confirm {
            return self.handle_delete_confirm(key);
        }
        if self.large_filtering {
            return self.handle_large_filter_input(key, out);
        }
        if self.entry_filtering {
            return self.handle_entry_filter_input(key, out);
        }

        match key {
            AnalyzeKey::Quit => AnalyzeEffect::Quit,
            AnalyzeKey::Esc => {
                if self.show_large_files {
                    if !self.large_filter.is_empty() {
                        self.large_filter.clear();
                        self.large_multi_selected.clear();
                        self.clamp_selection(out);
                        self.status.clear();
                        return AnalyzeEffect::None;
                    }
                    self.show_large_files = false;
                    self.large_multi_selected.clear();
                    self.selected = 0;
                    return AnalyzeEffect::None;
                }
                if !self.entry_filter.is_empty() {
                    self.entry_filter.clear();
                    self.multi_selected.clear();
                    self.clamp_selection(out);
                    self.status.clear();
                    return AnalyzeEffect::None;
                }
                if can_go_back {
                    AnalyzeEffect::GoBack
                } else {
                    AnalyzeEffect::Quit
                }
            }
            AnalyzeKey::Up => {
                self.selected = self.selected.saturating_sub(1);
                AnalyzeEffect::None
            }
            AnalyzeKey::Down => {
                let len = if self.show_large_files {
                    self.visible_large(out).len()
                } else {
                    self.visible_entries(out).len()
                };
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Enter => {
                if self.show_large_files {
                    return AnalyzeEffect::None;
                }
                let entries = self.visible_entries(out);
                if let Some(entry) = entries.get(self.selected) {
                    if entry.is_dir {
                        return AnalyzeEffect::EnterDir(entry.path.clone());
                    }
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Space => self.toggle_multi(out, scanning),
            AnalyzeKey::Filter => {
                if out.overview || scanning {
                    return AnalyzeEffect::None;
                }
                if self.show_large_files {
                    if out.large_files.is_empty() {
                        return AnalyzeEffect::None;
                    }
                    self.large_filtering = true;
                    self.status =
                        "Filter: type to match, Enter to apply, Esc to clear".to_string();
                } else if !out.entries.is_empty() {
                    self.entry_filtering = true;
                    self.status =
                        "Filter: type to match, Enter to apply, Esc to clear".to_string();
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Top => {
                if scanning || out.overview {
                    if scanning {
                        self.status =
                            "Top files are available after the scan finishes".to_string();
                    }
                    return AnalyzeEffect::None;
                }
                if out.large_files.is_empty() && !self.show_large_files {
                    return AnalyzeEffect::None;
                }
                self.show_large_files = !self.show_large_files;
                self.entry_filter.clear();
                self.large_filter.clear();
                self.entry_filtering = false;
                self.large_filtering = false;
                self.multi_selected.clear();
                self.large_multi_selected.clear();
                self.selected = 0;
                self.status.clear();
                AnalyzeEffect::None
            }
            AnalyzeKey::Delete => self.begin_delete(out, scanning),
            AnalyzeKey::Open => self.begin_open(out),
            AnalyzeKey::Preview => self.begin_preview(out),
            AnalyzeKey::FilterChar(_) | AnalyzeKey::FilterBackspace => AnalyzeEffect::None,
        }
    }

    fn handle_delete_confirm(&mut self, key: AnalyzeKey) -> AnalyzeEffect {
        match key {
            AnalyzeKey::Enter => {
                self.delete_confirm = false;
                AnalyzeEffect::ConfirmDelete
            }
            AnalyzeKey::Esc | AnalyzeKey::Quit => {
                self.delete_confirm = false;
                self.status = "Cancelled".to_string();
                AnalyzeEffect::CancelDelete
            }
            _ => AnalyzeEffect::None,
        }
    }

    fn handle_entry_filter_input(&mut self, key: AnalyzeKey, out: &AnalyzeOutput) -> AnalyzeEffect {
        match key {
            AnalyzeKey::Quit => AnalyzeEffect::Quit,
            AnalyzeKey::Esc => {
                self.entry_filtering = false;
                self.entry_filter.clear();
                self.multi_selected.clear();
                self.selected = 0;
                self.status.clear();
                AnalyzeEffect::None
            }
            AnalyzeKey::Enter => {
                self.entry_filtering = false;
                self.clamp_selection(out);
                self.status.clear();
                AnalyzeEffect::None
            }
            AnalyzeKey::FilterBackspace => {
                self.entry_filter.pop();
                self.multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            AnalyzeKey::Space => {
                self.entry_filter.push(' ');
                self.multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            AnalyzeKey::FilterChar(c) => {
                self.entry_filter.push(c);
                self.multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            _ => AnalyzeEffect::None,
        }
    }

    fn handle_large_filter_input(&mut self, key: AnalyzeKey, out: &AnalyzeOutput) -> AnalyzeEffect {
        match key {
            AnalyzeKey::Quit => AnalyzeEffect::Quit,
            AnalyzeKey::Esc => {
                self.large_filtering = false;
                self.large_filter.clear();
                self.large_multi_selected.clear();
                self.selected = 0;
                self.status.clear();
                AnalyzeEffect::None
            }
            AnalyzeKey::Enter => {
                self.large_filtering = false;
                self.clamp_selection(out);
                self.status.clear();
                AnalyzeEffect::None
            }
            AnalyzeKey::FilterBackspace => {
                self.large_filter.pop();
                self.large_multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            AnalyzeKey::Space => {
                self.large_filter.push(' ');
                self.large_multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            AnalyzeKey::FilterChar(c) => {
                self.large_filter.push(c);
                self.large_multi_selected.clear();
                self.selected = 0;
                AnalyzeEffect::None
            }
            _ => AnalyzeEffect::None,
        }
    }

    fn toggle_multi(&mut self, out: &AnalyzeOutput, scanning: bool) -> AnalyzeEffect {
        if scanning {
            self.status = "Selection is available after the scan finishes".to_string();
            return AnalyzeEffect::None;
        }
        if self.show_large_files {
            let items = self.visible_large(out);
            if let Some(item) = items.get(self.selected) {
                let path = item.path.clone();
                if !self.large_multi_selected.remove(&path) {
                    self.large_multi_selected.insert(path);
                }
                self.update_selection_status(out);
            }
            return AnalyzeEffect::None;
        }
        if out.overview {
            return AnalyzeEffect::None;
        }
        let items = self.visible_entries(out);
        if let Some(item) = items.get(self.selected) {
            let path = item.path.clone();
            if !self.multi_selected.remove(&path) {
                self.multi_selected.insert(path);
            }
            self.update_selection_status(out);
        }
        AnalyzeEffect::None
    }

    fn update_selection_status(&mut self, out: &AnalyzeOutput) {
        if self.show_large_files {
            let count = self.large_multi_selected.len();
            if count == 0 {
                self.status.clear();
                return;
            }
            let total: i64 = out
                .large_files
                .iter()
                .filter(|f| self.large_multi_selected.contains(&f.path))
                .map(|f| f.size.max(0))
                .sum();
            self.status = format!("{count} selected, {}", format_bytes_si(total as u64));
        } else {
            let count = self.multi_selected.len();
            if count == 0 {
                self.status.clear();
                return;
            }
            let total: i64 = out
                .entries
                .iter()
                .filter(|e| self.multi_selected.contains(&e.path))
                .map(|e| e.size.max(0))
                .sum();
            self.status = format!("{count} selected, {}", format_bytes_si(total as u64));
        }
    }

    fn begin_delete(&mut self, out: &AnalyzeOutput, scanning: bool) -> AnalyzeEffect {
        if scanning {
            self.status = "Delete is available after the scan finishes".to_string();
            return AnalyzeEffect::None;
        }
        let paths = self.paths_for_action(out);
        if paths.is_empty() {
            return AnalyzeEffect::None;
        }
        if self.show_large_files {
            // ok
        } else if out.overview {
            return AnalyzeEffect::None;
        }
        self.delete_confirm = true;
        AnalyzeEffect::RequestDelete(paths)
    }

    fn begin_open(&mut self, out: &AnalyzeOutput) -> AnalyzeEffect {
        const MAX_BATCH_OPEN: usize = 20;
        let paths = self.paths_for_action(out);
        if paths.is_empty() {
            return AnalyzeEffect::None;
        }
        if paths.len() > MAX_BATCH_OPEN {
            self.status = format!(
                "Too many items to open, max {MAX_BATCH_OPEN}, selected {}",
                paths.len()
            );
            return AnalyzeEffect::None;
        }
        AnalyzeEffect::Open(paths)
    }

    fn begin_preview(&mut self, out: &AnalyzeOutput) -> AnalyzeEffect {
        if self.show_large_files {
            let items = self.visible_large(out);
            if let Some(item) = items.get(self.selected) {
                return AnalyzeEffect::Preview(item.path.clone());
            }
            return AnalyzeEffect::None;
        }
        let items = self.visible_entries(out);
        if let Some(item) = items.get(self.selected) {
            if item.is_dir {
                return AnalyzeEffect::None;
            }
            return AnalyzeEffect::Preview(item.path.clone());
        }
        AnalyzeEffect::None
    }

    pub fn paths_for_action(&self, out: &AnalyzeOutput) -> Vec<String> {
        if self.show_large_files {
            if !self.large_multi_selected.is_empty() {
                return self.large_multi_selected.iter().cloned().collect();
            }
            return self
                .visible_large(out)
                .get(self.selected)
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default();
        }
        if !self.multi_selected.is_empty() {
            return self.multi_selected.iter().cloned().collect();
        }
        self.visible_entries(out)
            .get(self.selected)
            .map(|e| vec![e.path.clone()])
            .unwrap_or_default()
    }
}

fn format_bytes_si(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0usize;
    while v >= 1000.0 && unit + 1 < UNITS.len() {
        v /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{v:.1}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

    fn sample_out() -> AnalyzeOutput {
        AnalyzeOutput {
            path: "/tmp/a".into(),
            overview: false,
            total_size: 300,
            entries: vec![
                AnalyzeEntry {
                    name: "Caches".into(),
                    path: "/tmp/a/Caches".into(),
                    size: 200,
                    is_dir: true,
                    ..Default::default()
                },
                AnalyzeEntry {
                    name: "notes.txt".into(),
                    path: "/tmp/a/notes.txt".into(),
                    size: 100,
                    is_dir: false,
                    ..Default::default()
                },
            ],
            large_files: vec![AnalyzeFileEntry {
                name: "big.dmg".into(),
                path: "/tmp/a/big.dmg".into(),
                size: 1_000_000,
            }],
            total_files: Some(2),
        }
    }

    #[test]
    fn space_toggles_multi_select_after_scan() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        assert_eq!(
            st.handle_key(AnalyzeKey::Space, &out, true, false),
            AnalyzeEffect::None
        );
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.contains("/tmp/a/Caches"));
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.is_empty());
    }

    #[test]
    fn filter_applies_and_clears_selection() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(!st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Filter, &out, false, false);
        st.handle_key(AnalyzeKey::FilterChar('n'), &out, false, false);
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Enter, &out, false, false);
        let vis = st.visible_entries(&out);
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].name, "notes.txt");
        st.handle_key(AnalyzeKey::Esc, &out, false, false);
        assert_eq!(st.visible_entries(&out).len(), 2);
    }

    #[test]
    fn top_toggles_large_files_mode() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(st.show_large_files);
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(!st.show_large_files);
        st.handle_key(AnalyzeKey::Top, &out, true, false);
        assert!(!st.show_large_files);
    }

    #[test]
    fn overview_disables_space_filter_top() {
        let mut out = sample_out();
        out.overview = true;
        let mut st = AnalyzeState::default();
        st.handle_key(AnalyzeKey::Space, &out, false, false);
        assert!(st.multi_selected.is_empty());
        st.handle_key(AnalyzeKey::Filter, &out, false, false);
        assert!(!st.entry_filtering);
        st.handle_key(AnalyzeKey::Top, &out, false, false);
        assert!(!st.show_large_files);
    }
}
