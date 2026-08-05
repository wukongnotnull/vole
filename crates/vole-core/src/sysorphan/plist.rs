//! 读取 launchd plist 的 Program / ProgramArguments[0]。

use std::fs;
use std::path::{Path, PathBuf};

use plist::Value;

/// 先 `ProgramArguments[0]`，后 `Program`；非绝对路径或读失败 → `None`。
pub fn read_launchd_program(plist_path: &Path) -> Option<PathBuf> {
    let data = fs::read(plist_path).ok()?;
    let value = Value::from_reader(std::io::Cursor::new(data)).ok()?;
    let dict = value.as_dictionary()?;

    if let Some(args) = dict.get("ProgramArguments").and_then(Value::as_array) {
        if let Some(path) = absolute_program(args.first()) {
            return Some(path);
        }
    }

    absolute_program(dict.get("Program"))
}

fn absolute_program(value: Option<&Value>) -> Option<PathBuf> {
    let s = value?.as_string()?;
    if !s.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(tag: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("vole-sysorphan-plist-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_xml(path: &Path, body: &str) {
        fs::write(
            path,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
{body}
</dict>
</plist>
"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn prefers_program_arguments_zero() {
        let root = scratch("args");
        let plist = root.join("com.example.helper.plist");
        write_xml(
            &plist,
            r#"
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/first</string>
    <string>--flag</string>
  </array>
  <key>Program</key>
  <string>/usr/local/bin/second</string>
"#,
        );
        assert_eq!(
            read_launchd_program(&plist).as_deref(),
            Some(Path::new("/usr/local/bin/first"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn falls_back_to_program() {
        let root = scratch("prog");
        let plist = root.join("com.example.prog.plist");
        write_xml(
            &plist,
            r#"
  <key>Program</key>
  <string>/Library/PrivilegedHelperTools/com.example.Helper</string>
"#,
        );
        assert_eq!(
            read_launchd_program(&plist).as_deref(),
            Some(Path::new(
                "/Library/PrivilegedHelperTools/com.example.Helper"
            ))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_relative_empty_corrupt_unreadable() {
        let root = scratch("bad");
        let relative = root.join("relative.plist");
        write_xml(
            &relative,
            r#"
  <key>Program</key>
  <string>relative/bin</string>
"#,
        );
        assert!(read_launchd_program(&relative).is_none());

        let empty = root.join("empty.plist");
        write_xml(&empty, "");
        assert!(read_launchd_program(&empty).is_none());

        let corrupt = root.join("corrupt.plist");
        fs::write(&corrupt, b"not-a-plist").unwrap();
        assert!(read_launchd_program(&corrupt).is_none());

        let denied = root.join("denied.plist");
        write_xml(
            &denied,
            r#"
  <key>Program</key>
  <string>/bin/true</string>
"#,
        );
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).unwrap();
        let got = read_launchd_program(&denied);
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(got.is_none());

        let _ = fs::remove_dir_all(&root);
    }
}
