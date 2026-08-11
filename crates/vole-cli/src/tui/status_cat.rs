//! Animated ASCII vole frames (front-facing, inspired by the Vole logo).

/// Front-facing vole: round ears, whiskers, buck tooth, short tail.
/// Frames only vary eyes / feet for a gentle bob while sliding sideways.
const VOLE_BODY: &[&[&str]] = &[
    &[
        r#"     (\__/)"#,
        r#"   ≡( • • )≡"#,
        r#"    (  ▽T )"#,
        r#"     uu~uu"#,
    ],
    &[
        r#"     (\__/)"#,
        r#"   ≡( • • )≡"#,
        r#"    (  ▽T )"#,
        r#"     u u~u"#,
    ],
    &[
        r#"     (\__/)"#,
        r#"   ≡( · · )≡"#,
        r#"    (  ▽T )"#,
        r#"     uu~uu"#,
    ],
    &[
        r#"     (\__/)"#,
        r#"   ≡( • • )≡"#,
        r#"    (  ▽T )"#,
        r#"      uu~u"#,
    ],
];

/// Horizontal slide divisor: higher = slower travel across the status bar.
const MOVE_SLOWDOWN: u64 = 4;

/// Max display columns of the vole sprite (widest line).
const VOLE_WIDTH: usize = 12;

/// Render one animated vole frame (4 lines), facing the viewer while sliding.
pub fn render_mole_frame(anim_frame: u64, term_width: usize) -> String {
    let max_pos = term_width.saturating_sub(VOLE_WIDTH);
    let cycle_length = (max_pos * 2).max(1);
    let travel = (anim_frame / MOVE_SLOWDOWN) as usize;
    let mut pos = travel % cycle_length;
    if pos > max_pos {
        pos = cycle_length - pos;
    }

    let body = VOLE_BODY[(anim_frame as usize / MOVE_SLOWDOWN as usize) % VOLE_BODY.len()];
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
    fn mole_frame_contains_logo_cues_and_moves() {
        let a = render_mole_frame(0, 80);
        let b = render_mole_frame(MOVE_SLOWDOWN * 10, 80);
        assert!(a.contains(r"(\__/)"), "{a}");
        assert!(a.contains("≡("), "{a}");
        assert!(a.contains("▽T"), "{a}");
        assert!(a.contains("uu~uu"), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn mole_frame_stays_front_facing() {
        // Far into a leftward half of the cycle — still the same front sprite.
        let wide = 80usize;
        let max_pos = wide - VOLE_WIDTH;
        let leftward_frame = MOVE_SLOWDOWN * (max_pos as u64 + 5);
        let frame = render_mole_frame(leftward_frame, wide);
        assert!(frame.contains(r"(\__/)"), "{frame}");
        assert!(frame.contains("▽T"), "{frame}");
        assert!(frame.contains("≡("), "{frame}");
    }
}
