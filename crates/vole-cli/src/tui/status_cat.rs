//! Animated ASCII mole cat frames (ported from mole `cmd/status/view.go`).

const MOLE_BODY: &[&[&str]] = &[
    &[
        r#"     /\_/\"#,
        r#" ___/ o o \"#,
        r#"/___   =-= /"#,
        r#"\____)-m-m)"#,
    ],
    &[
        r#"     /\_/\"#,
        r#" ___/ o o \"#,
        r#"/___   =-= /"#,
        r#"\____)mm__)"#,
    ],
    &[
        r#"     /\_/\"#,
        r#" ___/ · · \"#,
        r#"/___   =-= /"#,
        r#"\___)-m__m)"#,
    ],
    &[
        r#"     /\_/\"#,
        r#" ___/ o o \"#,
        r#"/___   =-= /"#,
        r#"\____)-mm-)"#,
    ],
];

const MOLE_BODY_MIRROR: &[&[&str]] = &[
    &[
        r#"    /\_/\"#,
        r#"   / o o \___"#,
        r#"  \ =-=   ___\"#,
        r#"  (m-m-(____/"#,
    ],
    &[
        r#"    /\_/\"#,
        r#"   / o o \___"#,
        r#"  \ =-=   ___\"#,
        r#"  (__mm(____/"#,
    ],
    &[
        r#"    /\_/\"#,
        r#"   / · · \___"#,
        r#"  \ =-=   ___\"#,
        r#"  (m__m-(___/"#,
    ],
    &[
        r#"    /\_/\"#,
        r#"   / o o \___"#,
        r#"  \ =-=   ___\"#,
        r#"  (-mm-(____/"#,
    ],
];

/// Render one animated mole frame (4 lines), walking horizontally.
pub fn render_mole_frame(anim_frame: u64, term_width: usize) -> String {
    let mole_width = 15usize;
    let max_pos = term_width.saturating_sub(mole_width);
    let cycle_length = (max_pos * 2).max(1);
    let mut pos = (anim_frame as usize) % cycle_length;
    let moving_left = pos > max_pos;
    if moving_left {
        pos = cycle_length - pos;
    }

    let frames = if moving_left {
        MOLE_BODY_MIRROR
    } else {
        MOLE_BODY
    };
    let body = frames[(anim_frame as usize) % frames.len()];
    let padding = " ".repeat(pos);
    body.iter()
        .map(|line| format!("{padding}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mole_frame_contains_ears_and_moves() {
        let a = render_mole_frame(0, 80);
        let b = render_mole_frame(10, 80);
        assert!(a.contains(r"/\_/\"));
        assert_ne!(a, b);
    }
}
