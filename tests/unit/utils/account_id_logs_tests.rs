//! No log line prints a full IG account id.

use ig_client::utils::id::mask_account_id;

#[test]
fn test_mask_account_id_keeps_only_first_and_last_character() {
    assert_eq!(mask_account_id("ZR1234567W"), "Z…W");
    assert_eq!(mask_account_id("Z"), "…");
    assert_eq!(mask_account_id(""), "");
}

/// Code outside string literals, so a message that only mentions the words
/// does not count as printing the value.
fn without_strings(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in code.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// The code with every `mask_account_id(...)` call taken out, argument and all.
fn without_masked(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find("mask_account_id(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + "mask_account_id(".len()..];
        let mut depth = 1usize;
        let mut end = after.len();
        for (offset, c) in after.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = offset + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// A source check over `src/`: a log macro whose arguments name an account id
/// must pass it through `mask_account_id`. A log line cannot be intercepted
/// everywhere it is written, so the source is what is checked.
#[test]
fn test_no_log_line_prints_a_raw_account_id() {
    let log_macro = ["info!(", "warn!(", "error!(", "debug!(", "trace!("];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {dir:?}: {e}")) {
            let path = entry.unwrap_or_else(|e| panic!("list {dir:?}: {e}")).path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || !log_macro.iter().any(|m| code.contains(m)) {
                    continue;
                }
                // The macro call: from its line to the first `);`, at most a
                // few lines on.
                let end = (index + 8).min(lines.len());
                let joined = lines[index..end].join("\n");
                let call = joined.split(");").next().unwrap_or_default();
                let code_only = without_masked(&without_strings(call));
                // A field named account_id is fine; its value is what counts.
                let values = code_only.replace("account_id =", "");
                if values.contains("account_id") {
                    found.push(format!("{}:{}: {}", path.display(), index + 1, code));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "full account id in a log line:\n{}",
        found.join("\n")
    );
}
