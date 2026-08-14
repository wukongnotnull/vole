//! Pure menu logic for paginated multi-select (mole MenuContract A).

#![allow(dead_code)] // Public API; wired by paginated_select / uninstall interactive path.

use std::collections::HashSet;
use std::env;
use std::fmt;

use super::widgets::wrap_menu_block;

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub filter_name: Option<String>,
    pub epoch: Option<i64>,
    pub size_kb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Space,
    Enter,
    Quit,
    /// Return to the home menu (does not add to filter text).
    Back,
    Char(char),
    Backspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectOutcome {
    Confirmed(Vec<usize>),
    Cancelled,
    /// Leave this menu and reopen the bare `vole` home menu.
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Date,
    Name,
    Size,
}

#[derive(Debug, Clone)]
pub struct MenuConfig {
    pub sort_mode: SortMode,
    pub sort_reverse: bool,
    pub ignore_initial_enter: bool,
    pub preselected: Vec<usize>,
    pub term_height: u16,
    pub term_width: u16,
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self {
            sort_mode: SortMode::Date,
            sort_reverse: false,
            ignore_initial_enter: false,
            preselected: Vec::new(),
            term_height: 24,
            term_width: 80,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyMenuError;

impl fmt::Display for EmptyMenuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "No items provided")
    }
}

impl std::error::Error for EmptyMenuError {}

pub struct MenuState {
    items: Vec<MenuItem>,
    view_indices: Vec<usize>,
    selected: HashSet<usize>,
    /// Absolute index into `view_indices`.
    cursor: usize,
    /// Window start into `view_indices`.
    top: usize,
    filter_text: String,
    sort_mode: SortMode,
    sort_reverse: bool,
    ignore_initial_enter: bool,
    term_height: u16,
    term_width: u16,
}

impl MenuState {
    pub fn new(items: Vec<MenuItem>, cfg: MenuConfig) -> Result<Self, EmptyMenuError> {
        if items.is_empty() {
            return Err(EmptyMenuError);
        }

        let has_epoch = items.iter().any(|i| i.epoch.is_some());
        let has_size = items.iter().any(|i| i.size_kb.is_some());
        let sort_mode = resolve_sort_mode(cfg.sort_mode, has_epoch, has_size);

        let mut selected = HashSet::new();
        for idx in cfg.preselected {
            if idx < items.len() {
                selected.insert(idx);
            }
        }

        let mut state = Self {
            items,
            view_indices: Vec::new(),
            selected,
            cursor: 0,
            top: 0,
            filter_text: String::new(),
            sort_mode,
            sort_reverse: cfg.sort_reverse,
            ignore_initial_enter: cfg.ignore_initial_enter,
            term_height: cfg.term_height,
            term_width: cfg.term_width,
        };
        state.rebuild_view();
        Ok(state)
    }

    pub fn items_per_page(term_height: u16) -> usize {
        const RESERVED: u16 = 5;
        let available = term_height.saturating_sub(RESERVED) as usize;
        available.clamp(1, 50)
    }

    pub fn config_from_env() -> MenuConfig {
        let mut cfg = MenuConfig::default();
        if let Some(mode) = env_first(&["VOLE_MENU_SORT_MODE", "MOLE_MENU_SORT_MODE"]) {
            cfg.sort_mode = parse_sort_mode(&mode).unwrap_or(cfg.sort_mode);
        }
        if let Some(rev) = env_first(&["VOLE_MENU_SORT_REVERSE", "MOLE_MENU_SORT_REVERSE"]) {
            cfg.sort_reverse = parse_bool(&rev);
        }
        if let Some(ignore) = env_first(&[
            "VOLE_MENU_IGNORE_INITIAL_ENTER",
            "MOLE_MENU_IGNORE_INITIAL_ENTER",
        ]) {
            cfg.ignore_initial_enter = parse_bool(&ignore);
        }
        cfg
    }

    pub fn handle_key(&mut self, key: MenuKey) -> Option<SelectOutcome> {
        if self.ignore_initial_enter {
            self.ignore_initial_enter = false;
            if matches!(key, MenuKey::Enter) {
                return None;
            }
        }

        match key {
            MenuKey::Up => {
                self.move_up();
                None
            }
            MenuKey::Down => {
                self.move_down();
                None
            }
            MenuKey::Space => {
                self.toggle_current();
                None
            }
            MenuKey::Enter => {
                let mut idxs: Vec<usize> = self.selected.iter().copied().collect();
                idxs.sort_unstable();
                Some(SelectOutcome::Confirmed(idxs))
            }
            MenuKey::Quit => {
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.rebuild_view();
                    self.cursor = 0;
                    self.top = 0;
                    None
                } else {
                    Some(SelectOutcome::Cancelled)
                }
            }
            MenuKey::Back => {
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.rebuild_view();
                    self.cursor = 0;
                    self.top = 0;
                    None
                } else {
                    Some(SelectOutcome::Back)
                }
            }
            MenuKey::Char(c) => {
                if !c.is_control() {
                    self.filter_text.push(c);
                    self.rebuild_view();
                    self.cursor = 0;
                    self.top = 0;
                }
                None
            }
            MenuKey::Backspace => {
                if self.filter_text.pop().is_some() {
                    self.rebuild_view();
                    self.cursor = 0;
                    self.top = 0;
                }
                None
            }
        }
    }

    pub fn visible_page(&self) -> &[usize] {
        let end = self.page_end();
        &self.view_indices[self.top..end]
    }

    pub fn set_term_size(&mut self, width: u16, height: u16) {
        if width == self.term_width && height == self.term_height {
            return;
        }
        self.term_width = width.max(1);
        self.term_height = height.max(1);
        self.ensure_cursor_visible();
    }

    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    pub fn is_selected(&self, orig_idx: usize) -> bool {
        self.selected.contains(&orig_idx)
    }

    /// Cursor row within the current visible page (0-based).
    pub fn cursor_in_page(&self) -> usize {
        self.cursor.saturating_sub(self.top)
    }

    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    pub fn view_len(&self) -> usize {
        self.view_indices.len()
    }

    fn rebuild_view(&mut self) {
        let mut indices: Vec<usize> = (0..self.items.len()).collect();
        self.sort_indices(&mut indices);

        let filter_lower = self.filter_text.to_lowercase();
        if filter_lower.is_empty() {
            self.view_indices = indices;
        } else {
            self.view_indices = indices
                .into_iter()
                .filter(|&i| {
                    let target = self.items[i]
                        .filter_name
                        .as_deref()
                        .unwrap_or(self.items[i].label.as_str());
                    target.to_lowercase().contains(&filter_lower)
                })
                .collect();
        }

        if self.view_indices.is_empty() {
            self.cursor = 0;
            self.top = 0;
            return;
        }
        if self.cursor >= self.view_indices.len() {
            self.cursor = self.view_indices.len() - 1;
        }
        self.ensure_cursor_visible();
    }

    fn sort_indices(&self, indices: &mut [usize]) {
        match self.sort_mode {
            SortMode::Date => {
                indices.sort_by(|&a, &b| {
                    let ea = self.items[a].epoch.unwrap_or(0);
                    let eb = self.items[b].epoch.unwrap_or(0);
                    let ord = ea.cmp(&eb); // oldest first
                    if self.sort_reverse {
                        ord.reverse()
                    } else {
                        ord
                    }
                });
            }
            SortMode::Size => {
                indices.sort_by(|&a, &b| {
                    let sa = self.items[a].size_kb.unwrap_or(0);
                    let sb = self.items[b].size_kb.unwrap_or(0);
                    let ord = sb.cmp(&sa); // largest first
                    if self.sort_reverse {
                        ord.reverse()
                    } else {
                        ord
                    }
                });
            }
            SortMode::Name => {
                indices.sort_by(|&a, &b| {
                    let na = self.items[a].label.to_lowercase();
                    let nb = self.items[b].label.to_lowercase();
                    let ord = na.cmp(&nb).then_with(|| a.cmp(&b));
                    if self.sort_reverse {
                        ord.reverse()
                    } else {
                        ord
                    }
                });
            }
        }
    }

    fn move_up(&mut self) {
        if self.view_indices.is_empty() || self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.ensure_cursor_visible();
    }

    fn move_down(&mut self) {
        if self.view_indices.is_empty() {
            return;
        }
        let last = self.view_indices.len() - 1;
        if self.cursor < last {
            self.cursor += 1;
            self.ensure_cursor_visible();
        }
    }

    fn page_end(&self) -> usize {
        if self.view_indices.is_empty() {
            return 0;
        }
        let avail = Self::items_per_page(self.term_height);
        let mut used = 0usize;
        let mut end = self.top;
        let mut count = 0usize;
        while end < self.view_indices.len() && count < 50 {
            let h = self.visual_height(self.view_indices[end]);
            if count > 0 && used.saturating_add(h) > avail {
                break;
            }
            used = used.saturating_add(h);
            end += 1;
            count += 1;
        }
        end.max(self.top + 1).min(self.view_indices.len())
    }

    fn visual_height(&self, orig_idx: usize) -> usize {
        let item = &self.items[orig_idx];
        let size = item
            .size_kb
            .map(|kb| format!("  ({kb} KB)"))
            .unwrap_or_default();
        let lines: Vec<&str> = item.label.split('\n').collect();
        wrap_menu_block("[ ]", &lines, &size, self.term_width as usize)
            .len()
            .max(1)
    }

    fn ensure_cursor_visible(&mut self) {
        if self.view_indices.is_empty() {
            self.top = 0;
            return;
        }
        if self.cursor < self.top {
            self.top = self.cursor;
            return;
        }
        while self.cursor >= self.page_end() && self.top < self.cursor {
            self.top += 1;
        }
    }

    fn toggle_current(&mut self) {
        if self.view_indices.is_empty() {
            return;
        }
        let orig = self.view_indices[self.cursor];
        if !self.selected.remove(&orig) {
            self.selected.insert(orig);
        }
    }
}

fn resolve_sort_mode(requested: SortMode, has_epoch: bool, has_size: bool) -> SortMode {
    match requested {
        SortMode::Date if has_epoch => SortMode::Date,
        SortMode::Size if has_size => SortMode::Size,
        SortMode::Name => SortMode::Name,
        _ => SortMode::Name,
    }
}

fn env_first(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(v) = env::var(key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn parse_sort_mode(s: &str) -> Option<SortMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "date" => Some(SortMode::Date),
        "name" => Some(SortMode::Name),
        "size" => Some(SortMode::Size),
        _ => None,
    }
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_per_page_clamps() {
        assert_eq!(MenuState::items_per_page(3), 1);
        assert_eq!(MenuState::items_per_page(20), 15); // 20-5
        assert_eq!(MenuState::items_per_page(200), 50);
    }

    #[test]
    fn multiline_items_fill_page_by_row_height() {
        let items: Vec<MenuItem> = (0..10)
            .map(|i| MenuItem {
                label: format!("item{i}\nrepo  /a\npath  /b\nblockers  -"),
                filter_name: None,
                epoch: Some(i),
                size_kb: Some(1),
            })
            .collect();
        let st = MenuState::new(
            items,
            MenuConfig {
                term_height: 20,
                term_width: 80,
                ..MenuConfig::default()
            },
        )
        .unwrap();
        let page = st.visible_page();
        assert!(
            (1..=4).contains(&page.len()),
            "expected a few multi-line records, got {}",
            page.len()
        );
    }

    #[test]
    fn empty_menu_errors() {
        assert!(MenuState::new(vec![], MenuConfig::default()).is_err());
    }

    #[test]
    fn space_toggles_enter_returns_original_indices() {
        let items = vec![
            MenuItem {
                label: "B".into(),
                filter_name: None,
                epoch: Some(2),
                size_kb: Some(20),
            },
            MenuItem {
                label: "A".into(),
                filter_name: None,
                epoch: Some(1),
                size_kb: Some(10),
            },
        ];
        let mut st = MenuState::new(
            items,
            MenuConfig {
                sort_mode: SortMode::Name,
                ..MenuConfig::default()
            },
        )
        .unwrap();
        // name 排序后视图: A(1), B(0)
        assert_eq!(st.handle_key(MenuKey::Space), None);
        assert_eq!(
            st.handle_key(MenuKey::Enter),
            Some(SelectOutcome::Confirmed(vec![1]))
        );
    }

    #[test]
    fn quit_cancels_filter_clear_first() {
        let items = vec![MenuItem {
            label: "Alpha".into(),
            filter_name: None,
            epoch: None,
            size_kb: None,
        }];
        let mut st = MenuState::new(items, MenuConfig::default()).unwrap();
        st.handle_key(MenuKey::Char('a'));
        assert!(st.handle_key(MenuKey::Quit).is_none()); // 清过滤
        assert_eq!(st.handle_key(MenuKey::Quit), Some(SelectOutcome::Cancelled));
    }

    #[test]
    fn back_clears_filter_then_returns_home() {
        let items = vec![MenuItem {
            label: "Alpha".into(),
            filter_name: None,
            epoch: None,
            size_kb: None,
        }];
        let mut st = MenuState::new(items, MenuConfig::default()).unwrap();
        st.handle_key(MenuKey::Char('a'));
        assert!(st.handle_key(MenuKey::Back).is_none());
        assert_eq!(st.handle_key(MenuKey::Back), Some(SelectOutcome::Back));
    }

    #[test]
    fn ignore_initial_enter() {
        let items = vec![MenuItem {
            label: "X".into(),
            filter_name: None,
            epoch: None,
            size_kb: None,
        }];
        let mut st = MenuState::new(
            items,
            MenuConfig {
                ignore_initial_enter: true,
                ..MenuConfig::default()
            },
        )
        .unwrap();
        assert!(st.handle_key(MenuKey::Enter).is_none());
        st.handle_key(MenuKey::Space);
        assert_eq!(
            st.handle_key(MenuKey::Enter),
            Some(SelectOutcome::Confirmed(vec![0]))
        );
    }

    #[test]
    fn no_epoch_metadata_forces_name_sort() {
        let items = vec![
            MenuItem {
                label: "B".into(),
                filter_name: None,
                epoch: None,
                size_kb: Some(1),
            },
            MenuItem {
                label: "A".into(),
                filter_name: None,
                epoch: None,
                size_kb: Some(2),
            },
        ];
        let st = MenuState::new(
            items,
            MenuConfig {
                sort_mode: SortMode::Date,
                ..MenuConfig::default()
            },
        )
        .unwrap();
        assert_eq!(st.visible_page()[0], 1); // A
    }

    #[test]
    fn preselected_marks_initial_selection() {
        let items = vec![
            MenuItem {
                label: "A".into(),
                filter_name: None,
                epoch: None,
                size_kb: None,
            },
            MenuItem {
                label: "B".into(),
                filter_name: None,
                epoch: None,
                size_kb: None,
            },
        ];
        let mut st = MenuState::new(
            items,
            MenuConfig {
                preselected: vec![1],
                ..MenuConfig::default()
            },
        )
        .unwrap();
        assert!(st.is_selected(1));
        assert!(!st.is_selected(0));
        assert_eq!(
            st.handle_key(MenuKey::Enter),
            Some(SelectOutcome::Confirmed(vec![1]))
        );
    }

    #[test]
    fn size_sort_orders_largest_first() {
        let items = vec![
            MenuItem {
                label: "small".into(),
                filter_name: None,
                epoch: None,
                size_kb: Some(10),
            },
            MenuItem {
                label: "large".into(),
                filter_name: None,
                epoch: None,
                size_kb: Some(100),
            },
        ];
        let st = MenuState::new(
            items,
            MenuConfig {
                sort_mode: SortMode::Size,
                ..MenuConfig::default()
            },
        )
        .unwrap();
        assert_eq!(st.visible_page(), &[1, 0]);
    }
}
