//! 字节数格式化，对齐 Mole `internal/units`。

const KIB: u64 = 1024;
const MIB: u64 = KIB * 1024;
const GIB: u64 = MIB * 1024;
const TIB: u64 = GIB * 1024;

/// SI（1000 底），对齐 Finder/diskutil。负输入钳制为 "0 B"。
pub fn bytes_si(size: i64) -> String {
    if size < 0 {
        return "0 B".to_string();
    }
    let size = size as u64;
    const UNIT: u64 = 1000;
    if size < UNIT {
        return format!("{} B", size);
    }
    let mut div = UNIT;
    let mut exp = 0;
    let mut n = size / UNIT;
    while n >= UNIT {
        div *= UNIT;
        exp += 1;
        n /= UNIT;
    }
    let value = size as f64 / div as f64;
    let suffix = "kMGTPE".chars().nth(exp).unwrap_or('P');
    format!("{:.1} {}B", value, suffix)
}

/// 二进制（1024 底），带空格与单位标签。边界用 `>`，1024 仍为 "1024 B"。
pub fn bytes_bin(v: u64) -> String {
    match v {
        0 => "0 B".to_string(),
        v if v > TIB => format!("{:.1} TB", v as f64 / TIB as f64),
        v if v > GIB => format!("{:.1} GB", v as f64 / GIB as f64),
        v if v > MIB => format!("{:.1} MB", v as f64 / MIB as f64),
        v if v > KIB => format!("{:.1} KB", v as f64 / KIB as f64),
        v => format!("{} B", v),
    }
}

/// 二进制单位，无小数、无空格。边界用 `>=`。
pub fn bytes_bin_short(v: u64) -> String {
    match v {
        v if v >= TIB => format!("{:.0}T", v as f64 / TIB as f64),
        v if v >= GIB => format!("{:.0}G", v as f64 / GIB as f64),
        v if v >= MIB => format!("{:.0}M", v as f64 / MIB as f64),
        v if v >= KIB => format!("{:.0}K", v as f64 / KIB as f64),
        v => v.to_string(),
    }
}

/// 二进制单位，一位小数、无空格。边界用 `>=`。
pub fn bytes_bin_compact(v: u64) -> String {
    match v {
        v if v >= TIB => format!("{:.1}T", v as f64 / TIB as f64),
        v if v >= GIB => format!("{:.1}G", v as f64 / GIB as f64),
        v if v >= MIB => format!("{:.1}M", v as f64 / MIB as f64),
        v if v >= KIB => format!("{:.1}K", v as f64 / KIB as f64),
        v => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_si_matches_go() {
        let cases: &[(i64, &str)] = &[
            (-100, "0 B"),
            (0, "0 B"),
            (512, "512 B"),
            (999, "999 B"),
            (1000, "1.0 kB"),
            (1500, "1.5 kB"),
            (10000, "10.0 kB"),
            (1000000, "1.0 MB"),
            (1500000, "1.5 MB"),
            (1000000000, "1.0 GB"),
            (1000000000000, "1.0 TB"),
            (1000000000000000, "1.0 PB"),
        ];
        for (input, want) in cases {
            assert_eq!(bytes_si(*input), *want, "BytesSI({})", input);
        }
    }

    #[test]
    fn bytes_bin_matches_go() {
        let cases: &[(u64, &str)] = &[
            (0, "0 B"),
            (1, "1 B"),
            (1023, "1023 B"),
            (KIB, "1024 B"),
            (KIB + 1, "1.0 KB"),
            (1536, "1.5 KB"),
            (MIB, "1024.0 KB"),
            (MIB + 1, "1.0 MB"),
            (500 * MIB, "500.0 MB"),
            (GIB, "1024.0 MB"),
            (GIB + 1, "1.0 GB"),
            (100 * GIB, "100.0 GB"),
            (TIB, "1024.0 GB"),
            (TIB + 1, "1.0 TB"),
            (2 * TIB, "2.0 TB"),
        ];
        for (input, want) in cases {
            assert_eq!(bytes_bin(*input), *want, "BytesBin({})", input);
        }
    }

    #[test]
    fn bytes_bin_short_matches_go() {
        let cases: &[(u64, &str)] = &[
            (0, "0"),
            (1, "1"),
            (999, "999"),
            (KIB, "1K"),
            (KIB - 1, "1023"),
            (1536, "2K"),
            (999 * KIB, "999K"),
            (MIB, "1M"),
            (MIB - 1, "1024K"),
            (500 * MIB, "500M"),
            (GIB, "1G"),
            (GIB - 1, "1024M"),
            (100 * GIB, "100G"),
            (TIB, "1T"),
            (TIB - 1, "1024G"),
            (2 * TIB, "2T"),
        ];
        for (input, want) in cases {
            assert_eq!(bytes_bin_short(*input), *want, "BytesBinShort({})", input);
        }
    }

    #[test]
    fn bytes_bin_compact_matches_go() {
        let cases: &[(u64, &str)] = &[
            (0, "0"),
            (1, "1"),
            (1023, "1023"),
            (KIB, "1.0K"),
            (1536, "1.5K"),
            (MIB, "1.0M"),
            (500 * MIB, "500.0M"),
            (GIB, "1.0G"),
            (100 * GIB, "100.0G"),
            (TIB, "1.0T"),
            (2 * TIB, "2.0T"),
        ];
        for (input, want) in cases {
            assert_eq!(
                bytes_bin_compact(*input),
                *want,
                "BytesBinCompact({})",
                input
            );
        }
    }
}
