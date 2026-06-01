//! `lang_from_path` — map a file path to a language slug for syntax
//! highlighting. Terminal port of the web `langFromPath` (see
//! `web/src/components/CodeBlock.tsx`).
//!
//! The slug vocabulary matches the web Prism slugs verbatim; the TUI's
//! [`crate::tui::widgets::code_block`] resolves the slug to a syntect syntax
//! (and falls back to plain rendering when syntect has no matching syntax).
//!
//! Source for (shared `frontend/llr/`):
//! - `Code block highlights via Prism with extension map`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI CodeBlock syntect highlight rendering`

/// Map a file path to a fixed language slug, or `None` when the path's
/// basename has no known extension / special filename.
///
/// Resolution order (mirrors the web implementation):
/// 1. Strip to the basename (last `/` or `\` segment).
/// 2. Exact special-filename match (`Dockerfile`, `Makefile`, `.gitignore`,
///    `.bashrc`, `.zshrc`).
/// 3. Lowercased final extension via the extension map.
/// 4. `None` otherwise (no extension, trailing dot, or unknown extension).
pub fn lang_from_path(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return None;
    }
    // Basename = last path segment (handles both `/` and `\`).
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);

    if let Some(slug) = filename_map(base) {
        return Some(slug);
    }

    // Final extension: text after the last `.`. A leading-dot-only name
    // (e.g. `.gitignore`) was already handled by `filename_map`; here a
    // `.`-prefixed name with no further dot has `dot == 0`, which we reject.
    let dot = base.rfind('.')?;
    if dot == 0 || dot == base.len() - 1 {
        return None;
    }
    let ext = base[dot + 1..].to_ascii_lowercase();
    ext_map(&ext)
}

/// Fixed special-filename map.
fn filename_map(name: &str) -> Option<&'static str> {
    Some(match name {
        "Dockerfile" => "bash",
        "Makefile" => "bash",
        ".gitignore" => "bash",
        ".bashrc" => "bash",
        ".zshrc" => "bash",
        _ => return None,
    })
}

/// Fixed extension → slug map (lowercased extension).
fn ext_map(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" => "javascript",
        "mjs" => "javascript",
        "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "h" => "c",
        "cpp" => "cpp",
        "cc" => "cpp",
        "cxx" => "cpp",
        "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "sh" => "bash",
        "bash" => "bash",
        "zsh" => "bash",
        "yaml" => "yaml",
        "yml" => "yaml",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        "markdown" => "markdown",
        "html" => "markup",
        "htm" => "markup",
        "xml" => "markup",
        "svg" => "markup",
        "css" => "css",
        "scss" => "scss",
        "sql" => "sql",
        "diff" => "diff",
        "patch" => "diff",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_map_each_entry() {
        assert_eq!(lang_from_path("a/b.ts"), Some("typescript"));
        assert_eq!(lang_from_path("a.tsx"), Some("tsx"));
        assert_eq!(lang_from_path("a.js"), Some("javascript"));
        assert_eq!(lang_from_path("a.jsx"), Some("jsx"));
        assert_eq!(lang_from_path("a.py"), Some("python"));
        assert_eq!(lang_from_path("a.rs"), Some("rust"));
        assert_eq!(lang_from_path("a.go"), Some("go"));
        assert_eq!(lang_from_path("a.java"), Some("java"));
        assert_eq!(lang_from_path("a.c"), Some("c"));
        assert_eq!(lang_from_path("a.h"), Some("c"));
        assert_eq!(lang_from_path("a.cpp"), Some("cpp"));
        assert_eq!(lang_from_path("a.hpp"), Some("cpp"));
        assert_eq!(lang_from_path("a.cs"), Some("csharp"));
        assert_eq!(lang_from_path("a.rb"), Some("ruby"));
        assert_eq!(lang_from_path("a.sh"), Some("bash"));
        assert_eq!(lang_from_path("a.bash"), Some("bash"));
        assert_eq!(lang_from_path("a.zsh"), Some("bash"));
        assert_eq!(lang_from_path("a.yaml"), Some("yaml"));
        assert_eq!(lang_from_path("a.yml"), Some("yaml"));
        assert_eq!(lang_from_path("a.toml"), Some("toml"));
        assert_eq!(lang_from_path("a.json"), Some("json"));
        assert_eq!(lang_from_path("a.md"), Some("markdown"));
        assert_eq!(lang_from_path("a.markdown"), Some("markdown"));
        assert_eq!(lang_from_path("a.html"), Some("markup"));
        assert_eq!(lang_from_path("a.xml"), Some("markup"));
        assert_eq!(lang_from_path("a.css"), Some("css"));
        assert_eq!(lang_from_path("a.scss"), Some("scss"));
        assert_eq!(lang_from_path("a.sql"), Some("sql"));
    }

    #[test]
    fn filename_map_each_entry() {
        assert_eq!(lang_from_path("/x/Dockerfile"), Some("bash"));
        assert_eq!(lang_from_path("Makefile"), Some("bash"));
        assert_eq!(lang_from_path("repo/.gitignore"), Some("bash"));
        assert_eq!(lang_from_path(".bashrc"), Some("bash"));
        assert_eq!(lang_from_path(".zshrc"), Some("bash"));
    }

    #[test]
    fn unknown_extension_is_none() {
        assert_eq!(lang_from_path("a.zzz"), None);
        assert_eq!(lang_from_path("weird.unknownext"), None);
    }

    #[test]
    fn path_with_no_extension_is_none() {
        assert_eq!(lang_from_path("README"), None);
        assert_eq!(lang_from_path("/usr/bin/env"), None);
        assert_eq!(lang_from_path("trailingdot."), None);
        assert_eq!(lang_from_path(""), None);
    }

    #[test]
    fn case_insensitive_extension() {
        assert_eq!(lang_from_path("FOO.RS"), Some("rust"));
        assert_eq!(lang_from_path("Bar.Py"), Some("python"));
    }

    #[test]
    fn windows_separators() {
        assert_eq!(lang_from_path("C:\\src\\main.rs"), Some("rust"));
    }
}
