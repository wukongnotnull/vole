//! Mole-compatible status prefs (`~/.config/mole/status_prefs`).

use std::fs;
use std::path::PathBuf;

const CPU_CORES_CYCLE: [i32; 4] = [2, 4, 8, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusPrefs {
    pub cat_hidden: bool,
    /// 0 means show all cores.
    pub cpu_cores: i32,
}

impl Default for StatusPrefs {
    fn default() -> Self {
        Self {
            cat_hidden: false,
            cpu_cores: CPU_CORES_CYCLE[0],
        }
    }
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/mole/status_prefs"))
}

fn load_prefs_map() -> std::collections::BTreeMap<String, String> {
    let mut prefs = std::collections::BTreeMap::new();
    let Some(path) = config_path() else {
        return prefs;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return prefs;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        prefs.insert(key.trim().to_string(), value.trim().to_string());
    }
    prefs
}

fn save_pref(key: &str, value: &str) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let mut prefs = load_prefs_map();
    prefs.insert(key.to_string(), value.to_string());
    let mut out = String::new();
    for (k, v) in &prefs {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    let _ = fs::write(path, out);
}

pub fn load_status_prefs() -> StatusPrefs {
    let prefs = load_prefs_map();
    let cat_hidden = prefs
        .get("cat_hidden")
        .map(|v| v == "true")
        .unwrap_or(false);
    let cpu_cores = prefs
        .get("cpu_cores")
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|&n| n >= 0)
        .unwrap_or(CPU_CORES_CYCLE[0]);
    StatusPrefs {
        cat_hidden,
        cpu_cores,
    }
}

pub fn save_cat_hidden(hidden: bool) {
    save_pref("cat_hidden", if hidden { "true" } else { "false" });
}

pub fn save_cpu_cores(n: i32) {
    save_pref("cpu_cores", &n.to_string());
}

pub fn next_cpu_cores(current: i32) -> i32 {
    for (i, &v) in CPU_CORES_CYCLE.iter().enumerate() {
        if v == current {
            return CPU_CORES_CYCLE[(i + 1) % CPU_CORES_CYCLE.len()];
        }
    }
    CPU_CORES_CYCLE[0]
}

pub fn smaller_cpu_cores(current: i32) -> i32 {
    for (i, &v) in CPU_CORES_CYCLE.iter().enumerate() {
        if v == current {
            if i == 0 {
                return CPU_CORES_CYCLE[0];
            }
            return CPU_CORES_CYCLE[i - 1];
        }
    }
    CPU_CORES_CYCLE[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn next_cpu_cores_cycles() {
        assert_eq!(next_cpu_cores(2), 4);
        assert_eq!(next_cpu_cores(4), 8);
        assert_eq!(next_cpu_cores(8), 0);
        assert_eq!(next_cpu_cores(0), 2);
        assert_eq!(next_cpu_cores(99), 2);
    }

    #[test]
    fn smaller_cpu_cores_floors() {
        assert_eq!(smaller_cpu_cores(8), 4);
        assert_eq!(smaller_cpu_cores(2), 2);
        assert_eq!(smaller_cpu_cores(0), 8);
    }

    #[test]
    fn status_prefs_roundtrip() {
        let _guard = ENV_LOCK.lock().unwrap();
        let home = std::env::temp_dir().join(format!("vole-sp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("h")).unwrap();
        std::env::set_var("HOME", home.join("h"));

        save_cat_hidden(true);
        save_cpu_cores(8);
        let loaded = load_status_prefs();
        assert!(loaded.cat_hidden);
        assert_eq!(loaded.cpu_cores, 8);

        std::env::remove_var("HOME");
        let _ = std::fs::remove_dir_all(&home);
    }
}
