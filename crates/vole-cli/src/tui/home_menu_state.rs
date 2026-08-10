//! Pure home-menu cursor / key / footer logic (mole interactive_main_menu).

#![allow(dead_code)] // Public API; wired by home_menu / interactive.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeItem {
    pub title: &'static str,
    pub description: &'static str,
}

pub const HOME_ITEMS: [HomeItem; 5] = [
    HomeItem {
        title: "Clean",
        description: "Free up disk space",
    },
    HomeItem {
        title: "Uninstall",
        description: "Remove apps completely",
    },
    HomeItem {
        title: "Optimize",
        description: "Refresh caches and services",
    },
    HomeItem {
        title: "Analyze",
        description: "Explore disk usage",
    },
    HomeItem {
        title: "Status",
        description: "Monitor system health",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeCommand {
    Clean,
    Uninstall,
    Optimize,
    Analyze,
    Status,
    TouchId,
    Update,
}

impl HomeCommand {
    pub fn argv(self) -> &'static [&'static str] {
        match self {
            Self::Clean => &["clean"],
            Self::Uninstall => &["uninstall"],
            Self::Optimize => &["optimize"],
            Self::Analyze => &["analyze"],
            Self::Status => &["status"],
            Self::TouchId => &["touchid"],
            Self::Update => &["update"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeKey {
    Up,
    Down,
    Enter,
    Digit(u8),
    More,
    Version,
    TouchId,
    Update,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeAction {
    Launch(HomeCommand),
    ShowHelp,
    ShowVersion,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeMenuConfig {
    pub touchid_configured: bool,
    pub show_update: bool,
}

pub struct HomeMenuState {
    cursor: usize,
    cfg: HomeMenuConfig,
}

impl HomeMenuState {
    pub fn new(cfg: HomeMenuConfig) -> Self {
        Self { cursor: 0, cfg }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn cmd_at(idx: usize) -> HomeCommand {
        match idx {
            0 => HomeCommand::Clean,
            1 => HomeCommand::Uninstall,
            2 => HomeCommand::Optimize,
            3 => HomeCommand::Analyze,
            _ => HomeCommand::Status,
        }
    }

    pub fn handle_key(&mut self, key: HomeKey) -> Option<HomeAction> {
        match key {
            HomeKey::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                None
            }
            HomeKey::Down => {
                if self.cursor + 1 < 5 {
                    self.cursor += 1;
                }
                None
            }
            HomeKey::Enter => Some(HomeAction::Launch(Self::cmd_at(self.cursor))),
            HomeKey::Digit(d) if (1..=5).contains(&d) => {
                Some(HomeAction::Launch(Self::cmd_at((d - 1) as usize)))
            }
            HomeKey::More => Some(HomeAction::ShowHelp),
            HomeKey::Version => Some(HomeAction::ShowVersion),
            HomeKey::TouchId => Some(HomeAction::Launch(HomeCommand::TouchId)),
            HomeKey::Update if self.cfg.show_update => {
                Some(HomeAction::Launch(HomeCommand::Update))
            }
            HomeKey::Quit => Some(HomeAction::Quit),
            _ => None,
        }
    }

    pub fn footer_shows_touchid(&self) -> bool {
        !self.cfg.touchid_configured
    }

    pub fn footer_shows_update(&self) -> bool {
        self.cfg.touchid_configured && self.cfg.show_update
    }

    pub fn controls_line(&self) -> String {
        let mut s = String::from("↑↓  |  Enter  |  M More  |  V Version");
        if self.footer_shows_touchid() {
            s.push_str("  |  T TouchID");
        } else if self.footer_shows_update() {
            s.push_str("  |  U Update");
        }
        s.push_str("  |  Q Quit");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_items_match_mole_copy() {
        assert_eq!(HOME_ITEMS[0].title, "Clean");
        assert_eq!(HOME_ITEMS[0].description, "Free up disk space");
        assert_eq!(HOME_ITEMS[1].title, "Uninstall");
        assert_eq!(HOME_ITEMS[1].description, "Remove apps completely");
        assert_eq!(HOME_ITEMS[2].title, "Optimize");
        assert_eq!(HOME_ITEMS[2].description, "Refresh caches and services");
        assert_eq!(HOME_ITEMS[3].title, "Analyze");
        assert_eq!(HOME_ITEMS[3].description, "Explore disk usage");
        assert_eq!(HOME_ITEMS[4].title, "Status");
        assert_eq!(HOME_ITEMS[4].description, "Monitor system health");
    }

    #[test]
    fn arrows_wrap_not_and_digit_launches() {
        let mut st = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: true,
            show_update: false,
        });
        assert_eq!(st.cursor(), 0);
        assert!(st.handle_key(HomeKey::Up).is_none());
        assert_eq!(st.cursor(), 0);
        assert!(st.handle_key(HomeKey::Down).is_none());
        assert_eq!(st.cursor(), 1);
        assert_eq!(
            st.handle_key(HomeKey::Enter),
            Some(HomeAction::Launch(HomeCommand::Uninstall))
        );
        assert_eq!(
            st.handle_key(HomeKey::Digit(1)),
            Some(HomeAction::Launch(HomeCommand::Clean))
        );
        assert_eq!(HomeCommand::Clean.argv(), &["clean"]);
        assert_eq!(HomeCommand::Optimize.argv(), &["optimize"]);
    }

    #[test]
    fn m_v_q_and_conditional_u() {
        let mut st = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: true,
            show_update: false,
        });
        assert_eq!(st.handle_key(HomeKey::More), Some(HomeAction::ShowHelp));
        assert_eq!(
            st.handle_key(HomeKey::Version),
            Some(HomeAction::ShowVersion)
        );
        assert_eq!(st.handle_key(HomeKey::Quit), Some(HomeAction::Quit));
        assert!(st.handle_key(HomeKey::Update).is_none()); // 无更新条

        let mut st2 = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: true,
            show_update: true,
        });
        assert_eq!(
            st2.handle_key(HomeKey::Update),
            Some(HomeAction::Launch(HomeCommand::Update))
        );
    }

    #[test]
    fn footer_touchid_vs_update_elif() {
        let a = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: false,
            show_update: true,
        });
        assert!(a.footer_shows_touchid());
        assert!(!a.footer_shows_update()); // mole: T 优先，U 不进 footer
        let line = a.controls_line();
        assert!(line.contains("T TouchID"));
        assert!(!line.contains("U Update"));

        let b = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: true,
            show_update: true,
        });
        assert!(!b.footer_shows_touchid());
        assert!(b.footer_shows_update());
        let line = b.controls_line();
        assert!(!line.contains("T TouchID"));
        assert!(line.contains("U Update"));
    }

    #[test]
    fn touchid_key_always_launches_even_when_configured() {
        let mut st = HomeMenuState::new(HomeMenuConfig {
            touchid_configured: true,
            show_update: false,
        });
        assert_eq!(
            st.handle_key(HomeKey::TouchId),
            Some(HomeAction::Launch(HomeCommand::TouchId))
        );
        assert_eq!(HomeCommand::TouchId.argv(), &["touchid"]);
    }
}
