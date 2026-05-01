// Lightweight syntax highlighting via Prism. We import only a curated
// set of grammars so Vite tree-shakes the rest of prismjs out of the
// bundle. Adding a language here is a one-line side-effect import.
import Prism from "prismjs";

import "prismjs/components/prism-clike";
import "prismjs/components/prism-markup";
import "prismjs/components/prism-css";
import "prismjs/components/prism-javascript";
import "prismjs/components/prism-typescript";
import "prismjs/components/prism-jsx";
import "prismjs/components/prism-tsx";
import "prismjs/components/prism-json";
import "prismjs/components/prism-bash";
import "prismjs/components/prism-python";
import "prismjs/components/prism-rust";
import "prismjs/components/prism-go";
import "prismjs/components/prism-java";
import "prismjs/components/prism-c";
import "prismjs/components/prism-cpp";
import "prismjs/components/prism-csharp";
import "prismjs/components/prism-ruby";
import "prismjs/components/prism-sql";
import "prismjs/components/prism-yaml";
import "prismjs/components/prism-toml";
import "prismjs/components/prism-markdown";
import "prismjs/components/prism-diff";

// File extension → Prism language slug. Keep this small and obvious;
// extensions Prism doesn't natively know map to the closest grammar
// (e.g., .mjs / .cjs → javascript).
const EXT_TO_LANG: Record<string, string> = {
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  tsx: "tsx",
  json: "json",
  css: "css",
  scss: "css",
  html: "markup",
  htm: "markup",
  xml: "markup",
  svg: "markup",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  py: "python",
  rs: "rust",
  go: "go",
  java: "java",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  cs: "csharp",
  rb: "ruby",
  sql: "sql",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  md: "markdown",
  markdown: "markdown",
  diff: "diff",
  patch: "diff",
};

// Special filenames (no extension or fixed convention).
const FILENAME_TO_LANG: Record<string, string> = {
  Dockerfile: "bash",
  Makefile: "bash",
  ".gitignore": "bash",
  ".bashrc": "bash",
  ".zshrc": "bash",
};

export function langFromPath(path: string | null | undefined): string | null {
  if (!path) return null;
  const base = path.split(/[\\/]/).pop() ?? path;
  if (FILENAME_TO_LANG[base]) return FILENAME_TO_LANG[base];
  const dot = base.lastIndexOf(".");
  if (dot < 0 || dot === base.length - 1) return null;
  const ext = base.slice(dot + 1).toLowerCase();
  return EXT_TO_LANG[ext] ?? null;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function CodeBlock({
  text,
  language,
  className,
}: {
  text: string;
  language: string | null;
  className?: string;
}) {
  const grammar = language ? Prism.languages[language] : null;
  const html = grammar
    ? Prism.highlight(text, grammar, language!)
    : escapeHtml(text);
  const cls = ["code-block", className, language ? `language-${language}` : null]
    .filter(Boolean)
    .join(" ");
  return (
    <pre className={cls}>
      <code
        className={language ? `language-${language}` : undefined}
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </pre>
  );
}
