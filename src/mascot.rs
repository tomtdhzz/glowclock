//! The 胖猫 (fat-cat) mascot shown when a reminder pops.
//!
//! Pick a built-in cat by name (`--cat <name>`) or supply your own ASCII art
//! file (`--cat-file <path>`, one art row per line).

/// All built-in cat names, in menu order. `chonk` is the default.
pub const NAMES: &[&str] = &["chonk", "kitten", "loaf", "sleepy", "peek"];

/// The default cat used when none is chosen.
pub fn default_name() -> &'static str {
    "chonk"
}

/// Art rows for a built-in cat, or `None` if the name is unknown.
pub fn builtin(name: &str) -> Option<Vec<String>> {
    let art: &[&str] = match name {
        // A round, content chonky cat — one continuous silhouette.
        "chonk" => &[" /\\_/\\", "( o.o )", "(  w  )", "(     )", "(_____)"],
        // Tiny happy kitten.
        "kitten" => &[" /\\_/\\", "( ^.^ )", " (u_u) "],
        // The classic cute loaf cat.
        "loaf" => &[" ╱|、", "(˚ˎ 。7", " |、˜〵", " じしˍ,)ノ"],
        // The iconic sleeping cat.
        "sleepy" => &[
            "      |\\      _,,,---,,_",
            "      /,`.-'`'    -.  ;-;;,_",
            "     |,4-  ) )-,_..;\\ (  `'-'",
            "    '---''(_/--'  `-'\\_)",
        ],
        // A curious cat peeking over an edge.
        "peek" => &[
            "  /\\     /\\",
            " (  . _ .  )",
            "  o  ___  o",
            " /|_|___|_|\\",
        ],
        _ => return None,
    };
    Some(art.iter().map(|s| s.to_string()).collect())
}

/// Resolve a cat: explicit file wins, then a built-in name, then the default.
pub fn resolve(name: Option<&str>, file: Option<&str>) -> Result<Vec<String>, String> {
    if let Some(p) = file {
        let text =
            std::fs::read_to_string(p).map_err(|e| format!("cannot read cat file {p}: {e}"))?;
        let lines: Vec<String> = text
            .lines()
            .map(|l| l.trim_end_matches(['\r', '\n']).to_string())
            .collect();
        if lines.iter().all(|l| l.trim().is_empty()) {
            return Err(format!("cat file {p} has no art"));
        }
        return Ok(lines);
    }
    let name = name.unwrap_or(default_name());
    builtin(name).ok_or_else(|| format!("unknown cat: {name} (try: {})", NAMES.join(", ")))
}

/// Approximate display width of `s` in terminal columns (CJK/full-width glyphs
/// count as two). Good enough for sizing the popup box.
pub fn disp_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            if (c as u32) > 0x1100 && !c.is_ascii() {
                2
            } else {
                1
            }
        })
        .sum()
}

/// Widest row of `cat`, in display columns.
pub fn width(cat: &[String]) -> usize {
    cat.iter().map(|l| disp_width(l)).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_cat_resolves() {
        for n in NAMES {
            let cat = resolve(Some(n), None).expect("named cat resolves");
            assert!(!cat.is_empty());
            assert!(width(&cat) > 0);
        }
    }

    #[test]
    fn default_is_a_valid_name() {
        assert!(NAMES.contains(&default_name()));
        assert!(resolve(None, None).is_ok());
    }

    #[test]
    fn unknown_cat_is_an_error() {
        assert!(resolve(Some("dog"), None).is_err());
    }
}
