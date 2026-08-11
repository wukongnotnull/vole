//! analyze TUI 状态机（Space / Filter / Top / Delete 确认等纯逻辑）。

use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vole_core::vole_proto::{AnalyzeEntry, AnalyzeFileEntry, AnalyzeOutput};

use super::analyze_actions::MAX_BATCH_OPEN;
use super::widgets::AnalyzeFooterMode;

pub fn map_analyze_key(key: KeyEvent, filtering: bool) -> Option<AnalyzeKey> {
    if filtering {
        return match key.code {
            KeyCode::Esc => Some(AnalyzeKey::Esc),
            KeyCode::Enter => Some(AnalyzeKey::Enter),
            KeyCode::Backspace | KeyCode::Delete => Some(AnalyzeKey::FilterBackspace),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(AnalyzeKey::Quit)
            }
            KeyCode::Char(' ') => Some(AnalyzeKey::Space),
            KeyCode::Char(c) => Some(AnalyzeKey::FilterChar(c)),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Up => Some(AnalyzeKey::Up),
        KeyCode::Down => Some(AnalyzeKey::Down),
        KeyCode::Left => Some(AnalyzeKey::Back),
        KeyCode::Right => Some(AnalyzeKey::Forward),
        KeyCode::Enter => Some(AnalyzeKey::Enter),
        KeyCode::Esc => Some(AnalyzeKey::Esc),
        KeyCode::Backspace | KeyCode::Delete => Some(AnalyzeKey::Delete),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(AnalyzeKey::Quit)
        }
        KeyCode::Char(' ') => Some(AnalyzeKey::Space),
        KeyCode::Char('/') => Some(AnalyzeKey::Filter),
        KeyCode::Char(c) => match c {
            'q' | 'Q' => Some(AnalyzeKey::Quit),
            'o' | 'O' => Some(AnalyzeKey::Open),
            'p' | 'P' => Some(AnalyzeKey::Preview),
            't' | 'T' => Some(AnalyzeKey::Top),
            'f' | 'F' => Some(AnalyzeKey::Reveal),
            'r' | 'R' => Some(AnalyzeKey::Refresh),
            's' | 'S' => Some(AnalyzeKey::LiveSort),
            'h' | 'H' | 'b' | 'B' => Some(AnalyzeKey::Back),
            'l' | 'L' => Some(AnalyzeKey::Forward),
            _ => None,
        },
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveSortMode {
    FreezeOnMove,
    Continuous,
}

pub fn live_sort_mode_from_env() -> LiveSortMode {
    let raw = std::env::var("VOLE_ANALYZE_LIVE_SORT")
        .or_else(|_| std::env::var("MOLE_ANALYZE_LIVE_SORT"))
        .unwrap_or_default();
    match raw.to_ascii_lowercase().trim() {
        "continuous" => LiveSortMode::Continuous,
        _ => LiveSortMode::FreezeOnMove,
    }
}

pub fn next_live_sort_mode(mode: LiveSortMode) -> LiveSortMode {
    match mode {
        LiveSortMode::Continuous => LiveSortMode::FreezeOnMove,
        LiveSortMode::FreezeOnMove => LiveSortMode::Continuous,
    }
}

pub fn live_sort_mode_label(mode: LiveSortMode) -> &'static str {
    match mode {
        LiveSortMode::FreezeOnMove => "freeze-on-move",
        LiveSortMode::Continuous => "continuous",
    }
}

pub fn sort_entries_by_size(entries: &mut [AnalyzeEntry]) {
    entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
}

pub fn upsert_live_child(out: &mut AnalyzeOutput, child: AnalyzeEntry) {
    if let Some(i) = out.entries.iter().position(|e| e.path == child.path) {
        let prev = out.entries[i].size;
        out.total_size += child.size - prev;
        out.entries[i] = child;
    } else {
        out.total_size += child.size;
        out.entries.push(child);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeKey {
    Up,
    Down,
    Enter,
    Esc,
    Back,
    Forward,
    Quit,
    Space,
    Delete,
    Open,
    Preview,
    Reveal,
    Refresh,
    LiveSort,
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
    Reveal(Vec<String>),
    Refresh,
    RequestDelete(Vec<String>),
    ConfirmDelete,
    CancelDelete,
}

#[derive(Debug, Clone)]
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
    pub live_sort_mode: LiveSortMode,
    pub auto_sort_live: bool,
}

impl Default for AnalyzeState {
    fn default() -> Self {
        Self {
            selected: 0,
            show_large_files: false,
            multi_selected: BTreeSet::new(),
            large_multi_selected: BTreeSet::new(),
            entry_filter: String::new(),
            large_filter: String::new(),
            entry_filtering: false,
            large_filtering: false,
            delete_confirm: false,
            status: String::new(),
            live_sort_mode: live_sort_mode_from_env(),
            auto_sort_live: false,
        }
    }
}

impl AnalyzeState {
    pub fn begin_live_scan(&mut self) {
        self.auto_sort_live = true;
    }

    pub fn note_live_cursor_move(&mut self, scanning: bool) {
        if scanning && !self.show_large_files && self.live_sort_mode == LiveSortMode::FreezeOnMove {
            self.auto_sort_live = false;
        }
    }

    pub fn apply_live_sort_after_progress(&mut self, out: &mut AnalyzeOutput) {
        if !self.auto_sort_live {
            return;
        }
        let selected_path = if self.live_sort_mode == LiveSortMode::Continuous {
            self.visible_entries(out)
                .get(self.selected)
                .map(|e| e.path.clone())
        } else {
            None
        };
        sort_entries_by_size(&mut out.entries);
        match self.live_sort_mode {
            LiveSortMode::FreezeOnMove => {
                self.selected = 0;
            }
            LiveSortMode::Continuous => {
                if let Some(path) = selected_path {
                    if let Some(i) = out.entries.iter().position(|e| e.path == path) {
                        self.selected = i;
                    }
                }
            }
        }
    }

    /// Returns whether selection should pin to row 0 after Done (mole pinFirstRow).
    pub fn take_live_scan_pin_first(&mut self) -> bool {
        let pin = self.live_sort_mode == LiveSortMode::FreezeOnMove && self.auto_sort_live;
        self.auto_sort_live = false;
        pin
    }

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
            AnalyzeKey::Esc | AnalyzeKey::Back => {
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
                let prev = self.selected;
                self.selected = self.selected.saturating_sub(1);
                if prev != self.selected {
                    self.note_live_cursor_move(scanning);
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Down => {
                let len = if self.show_large_files {
                    self.visible_large(out).len()
                } else {
                    self.visible_entries(out).len()
                };
                let prev = self.selected;
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                if prev != self.selected {
                    self.note_live_cursor_move(scanning);
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Enter | AnalyzeKey::Forward => {
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
            AnalyzeKey::LiveSort => {
                if !scanning || out.overview || self.show_large_files {
                    return AnalyzeEffect::None;
                }
                self.live_sort_mode = next_live_sort_mode(self.live_sort_mode);
                self.auto_sort_live = self.live_sort_mode == LiveSortMode::Continuous;
                self.status = format!("Live sort: {}", live_sort_mode_label(self.live_sort_mode));
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
                    self.status = "Filter: type to match, Enter to apply, Esc to clear".to_string();
                } else if !out.entries.is_empty() {
                    self.entry_filtering = true;
                    self.status = "Filter: type to match, Enter to apply, Esc to clear".to_string();
                }
                AnalyzeEffect::None
            }
            AnalyzeKey::Top => {
                if scanning || out.overview {
                    if scanning {
                        self.status = "Top files are available after the scan finishes".to_string();
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
            AnalyzeKey::Reveal => self.begin_reveal(out),
            AnalyzeKey::Refresh => {
                self.multi_selected.clear();
                self.large_multi_selected.clear();
                self.status = "Refreshing...".to_string();
                AnalyzeEffect::Refresh
            }
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

    fn begin_reveal(&mut self, out: &AnalyzeOutput) -> AnalyzeEffect {
        let paths = self.paths_for_action(out);
        if paths.is_empty() {
            return AnalyzeEffect::None;
        }
        if paths.len() > MAX_BATCH_OPEN {
            self.status = format!(
                "Too many items to reveal, max {MAX_BATCH_OPEN}, selected {}",
                paths.len()
            );
            return AnalyzeEffect::None;
        }
        AnalyzeEffect::Reveal(paths)
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

    pub fn footer_mode(&self, out: &AnalyzeOutput, can_go_back: bool) -> AnalyzeFooterMode {
        if self.delete_confirm {
            return AnalyzeFooterMode::DeleteConfirm;
        }
        if self.entry_filtering || self.large_filtering {
            return AnalyzeFooterMode::Filtering;
        }
        if self.show_large_files {
            return AnalyzeFooterMode::Top {
                selected_count: self.large_multi_selected.len(),
            };
        }
        AnalyzeFooterMode::Directory {
            can_go_back,
            selected_count: self.multi_selected.len(),
            large_count: out.large_files.len(),
        }
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

    #[test]
    fn delete_enters_confirm_then_cancel() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        let eff = st.handle_key(AnalyzeKey::Delete, &out, false, false);
        assert!(matches!(eff, AnalyzeEffect::RequestDelete(_)));
        assert!(st.delete_confirm);
        st.handle_key(AnalyzeKey::Esc, &out, false, false);
        assert!(!st.delete_confirm);
    }

    #[test]
    fn map_analyze_key_core_bindings() {
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Space)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), false),
            Some(AnalyzeKey::Delete)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Filter)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), true),
            Some(AnalyzeKey::FilterChar('n'))
        );
    }

    #[test]
    fn open_caps_batch_at_20() {
        let mut out = sample_out();
        out.entries = (0..25)
            .map(|i| AnalyzeEntry {
                name: format!("f{i}"),
                path: format!("/tmp/a/f{i}"),
                size: 1,
                is_dir: false,
                ..Default::default()
            })
            .collect();
        let mut st = AnalyzeState::default();
        for i in 0..25 {
            st.selected = i;
            st.handle_key(AnalyzeKey::Space, &out, false, false);
        }
        let eff = st.handle_key(AnalyzeKey::Open, &out, false, false);
        assert_eq!(eff, AnalyzeEffect::None);
        assert!(st.status.contains("max 20"));
    }

    #[test]
    fn map_key_reveal_and_refresh() {
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Reveal)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Refresh)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), true),
            Some(AnalyzeKey::FilterChar('f'))
        );
    }

    #[test]
    fn reveal_respects_batch_limit_and_refresh_effect() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        for i in 0..21 {
            st.multi_selected.insert(format!("/tmp/a/item{i}"));
        }
        assert_eq!(
            st.handle_key(AnalyzeKey::Reveal, &out, false, false),
            AnalyzeEffect::None
        );
        assert!(st.status.contains("Too many items to reveal"));

        st.multi_selected.clear();
        st.multi_selected.insert("/tmp/a/Caches".into());
        assert_eq!(
            st.handle_key(AnalyzeKey::Reveal, &out, false, false),
            AnalyzeEffect::Reveal(vec!["/tmp/a/Caches".into()])
        );

        st.delete_confirm = true;
        assert_eq!(
            st.handle_key(AnalyzeKey::Refresh, &out, false, false),
            AnalyzeEffect::None
        );

        st.delete_confirm = false;
        assert_eq!(
            st.handle_key(AnalyzeKey::Refresh, &out, false, false),
            AnalyzeEffect::Refresh
        );
    }

    #[test]
    fn map_key_nav_aliases_and_live_sort() {
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), false),
            Some(AnalyzeKey::Back)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Back)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), false),
            Some(AnalyzeKey::Forward)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::Forward)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), false),
            Some(AnalyzeKey::LiveSort)
        );
        assert_eq!(
            map_analyze_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), true),
            Some(AnalyzeKey::FilterChar('h'))
        );
    }

    #[test]
    fn back_and_forward_match_esc_and_enter() {
        let out = sample_out();
        let mut st = AnalyzeState::default();
        assert_eq!(
            st.handle_key(AnalyzeKey::Forward, &out, false, false),
            AnalyzeEffect::EnterDir("/tmp/a/Caches".into())
        );
        assert_eq!(
            st.handle_key(AnalyzeKey::Back, &out, false, true),
            AnalyzeEffect::GoBack
        );
        st.show_large_files = true;
        assert_eq!(
            st.handle_key(AnalyzeKey::Back, &out, false, true),
            AnalyzeEffect::None
        );
        assert!(!st.show_large_files);
    }

    #[test]
    fn live_sort_toggles_only_while_scanning() {
        let out = sample_out();
        let mut st = AnalyzeState {
            live_sort_mode: LiveSortMode::FreezeOnMove,
            ..AnalyzeState::default()
        };
        st.handle_key(AnalyzeKey::LiveSort, &out, false, false);
        assert_eq!(st.live_sort_mode, LiveSortMode::FreezeOnMove);
        st.handle_key(AnalyzeKey::LiveSort, &out, true, false);
        assert_eq!(st.live_sort_mode, LiveSortMode::Continuous);
        assert!(st.auto_sort_live);
        assert!(st.status.contains("continuous"));
        st.handle_key(AnalyzeKey::LiveSort, &out, true, false);
        assert_eq!(st.live_sort_mode, LiveSortMode::FreezeOnMove);
        assert!(!st.auto_sort_live);
    }

    #[test]
    fn freeze_on_move_stops_after_effective_down() {
        let out = sample_out();
        let mut st = AnalyzeState {
            live_sort_mode: LiveSortMode::FreezeOnMove,
            ..AnalyzeState::default()
        };
        st.begin_live_scan();
        assert!(st.auto_sort_live);
        st.handle_key(AnalyzeKey::Down, &out, true, false);
        assert!(!st.auto_sort_live);
        let mut st = AnalyzeState {
            live_sort_mode: LiveSortMode::FreezeOnMove,
            auto_sort_live: true,
            selected: out.entries.len() - 1,
            ..AnalyzeState::default()
        };
        st.handle_key(AnalyzeKey::Down, &out, true, false);
        assert!(st.auto_sort_live, "boundary down must not freeze");
    }

    #[test]
    fn continuous_keeps_selected_path_after_sort() {
        let mut out = sample_out();
        // smaller first so sort will move Caches to index 0
        out.entries = vec![
            AnalyzeEntry {
                name: "notes.txt".into(),
                path: "/tmp/a/notes.txt".into(),
                size: 100,
                is_dir: false,
                ..Default::default()
            },
            AnalyzeEntry {
                name: "Caches".into(),
                path: "/tmp/a/Caches".into(),
                size: 200,
                is_dir: true,
                ..Default::default()
            },
        ];
        let mut st = AnalyzeState {
            live_sort_mode: LiveSortMode::Continuous,
            auto_sort_live: true,
            selected: 1, // Caches
            ..AnalyzeState::default()
        };
        st.apply_live_sort_after_progress(&mut out);
        assert_eq!(out.entries[0].name, "Caches");
        assert_eq!(st.selected, 0);
    }
}
