//! Разбор unified diff на блоки для выборочной подготовки.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub patch: String,
}

/// Разбить вывод `git diff` на блоки `@@ … @@`.
pub fn parse_patch_hunks(text: &str) -> Vec<Hunk> {
    let lines: Vec<&str> = text.lines().collect();
    let mut prefix = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("@@") {
            break;
        }
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("new file")
            || line.starts_with("deleted file")
            || line.starts_with("similarity index")
            || line.starts_with("rename ")
        {
            if !prefix.is_empty() {
                prefix.push('\n');
            }
            prefix.push_str(line);
            i += 1;
        } else if prefix.is_empty() && line.trim().is_empty() {
            i += 1;
        } else {
            break;
        }
    }
    let mut hunks = Vec::new();
    while i < lines.len() {
        if !lines[i].starts_with("@@") {
            i += 1;
            continue;
        }
        let header = lines[i].to_string();
        let mut patch = String::new();
        if !prefix.is_empty() {
            patch.push_str(&prefix);
            patch.push('\n');
        }
        patch.push_str(lines[i]);
        patch.push('\n');
        i += 1;
        while i < lines.len() && !lines[i].starts_with("@@") {
            patch.push_str(lines[i]);
            patch.push('\n');
            i += 1;
        }
        hunks.push(Hunk { header, patch });
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_two_hunks_with_file_headers() {
        let text = r#"diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,3 +1,4 @@
 fn main() {
+    let x = 1;
     println!("hi");
 }
@@ -10,2 +11,3 @@ fn other() {
     foo();
+    bar();
 }
"#;
        let hunks = parse_patch_hunks(text);
        assert_eq!(hunks.len(), 2);
        assert!(hunks[0].patch.contains("--- a/src/a.rs"));
        assert!(hunks[0].header.contains("-1,3"));
        assert!(hunks[1].patch.contains("+    bar();"));
    }
}