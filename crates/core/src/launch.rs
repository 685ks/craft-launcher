//! Per-app launch profiles: extra command-line arguments and environment variables an app is
//! started with, typed by the user in the app's "Launch options" dialog.
//!
//! The profile is kept as the user typed it ([`LaunchProfile`]), so the dialog shows it back
//! unchanged, and parsed ([`LaunchProfile::parse`]) each time the app starts. Parsing is strict
//! (an unclosed quote or a bad variable name is an error, not a guess) and bounded.

use serde::{Deserialize, Serialize};

/// The most arguments a profile may hold.
pub const MAX_ARGS: usize = 64;
/// The most environment variables a profile may set.
pub const MAX_VARS: usize = 64;
/// The longest the arguments line or the variables text may be, in bytes.
pub const MAX_TEXT: usize = 8 * 1024;

/// What the user typed for one app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchProfile {
    /// Arguments on one line, split like a shell does: spaces separate them, `'…'` and `"…"`
    /// quote, a backslash escapes the next character (outside single quotes).
    pub args: String,
    /// One `NAME=value` per line; blank lines and lines starting with `#` are skipped.
    pub env: String,
}

/// A profile ready to start an app with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl LaunchOptions {
    pub fn is_empty(&self) -> bool {
        self.args.is_empty() && self.env.is_empty()
    }
}

impl LaunchProfile {
    /// Nothing typed: the app starts as it always does.
    pub fn is_empty(&self) -> bool {
        self.args.trim().is_empty() && parse_env(&self.env).is_ok_and(|v| v.is_empty())
    }

    pub fn parse(&self) -> Result<LaunchOptions, String> {
        Ok(LaunchOptions { args: split_args(&self.args)?, env: parse_env(&self.env)? })
    }
}

/// Splits a line of arguments the way a POSIX shell would (quotes and backslashes only: no
/// variables, globs or other expansion).
pub fn split_args(line: &str) -> Result<Vec<String>, String> {
    if line.len() > MAX_TEXT {
        return Err("the arguments are too long".into());
    }
    let mut args = Vec::new();
    let mut current = String::new();
    // Whether `current` is an argument, even an empty one (`""`).
    let mut started = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => current.push(c),
                        None => return Err("a quote (') isn't closed".into()),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => current.push(c),
                            Some(c) => {
                                current.push('\\');
                                current.push(c);
                            }
                            None => return Err("a quote (\") isn't closed".into()),
                        },
                        Some(c) => current.push(c),
                        None => return Err("a quote (\") isn't closed".into()),
                    }
                }
            }
            '\\' => {
                started = true;
                match chars.next() {
                    Some(c) => current.push(c),
                    None => return Err("the arguments end with a lone backslash".into()),
                }
            }
            c if c.is_control() => return Err("the arguments contain a control character".into()),
            c => {
                started = true;
                current.push(c);
            }
        }
        if args.len() > MAX_ARGS {
            return Err(format!("more than {MAX_ARGS} arguments"));
        }
    }
    if started {
        args.push(current);
    }
    if args.len() > MAX_ARGS {
        return Err(format!("more than {MAX_ARGS} arguments"));
    }
    Ok(args)
}

/// Whether `name` is a portable environment variable name: a letter or `_`, then letters,
/// digits and `_`.
pub fn valid_var_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic() || b == b'_') && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_') && name.len() <= 128
}

/// Reads `NAME=value` lines. The value is everything after the first `=`, as typed (no quotes
/// are taken off). A name set twice keeps its last value.
pub fn parse_env(text: &str) -> Result<Vec<(String, String)>, String> {
    if text.len() > MAX_TEXT {
        return Err("the environment variables are too long".into());
    }
    let mut vars: Vec<(String, String)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_start();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let line_no = (n + 1).to_string();
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("line {line_no} isn't NAME=value"));
        };
        let name = name.trim_end();
        if !valid_var_name(name) {
            return Err(format!("line {line_no}: {name} isn't a valid variable name"));
        }
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return Err(format!("line {line_no}: the value contains a control character"));
        }
        vars.retain(|(n, _)| n != name);
        vars.push((name.to_string(), value.trim_end_matches(['\r']).to_string()));
        if vars.len() > MAX_VARS {
            return Err(format!("more than {MAX_VARS} variables"));
        }
    }
    Ok(vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn arguments_split_like_a_shell() {
        assert_eq!(split_args("").unwrap(), Vec::<String>::new());
        assert_eq!(split_args("  --verbose   -n 3 ").unwrap(), s(&["--verbose", "-n", "3"]));
        assert_eq!(split_args(r#"--open "My File.png" 'it''s'"#).unwrap(), s(&["--open", "My File.png", "its"]));
        assert_eq!(split_args(r#"a\ b "x\"y" "\n" '' """#).unwrap(), s(&["a b", "x\"y", "\\n", "", ""]));
        assert_eq!(split_args(r"C:\\Users\\me").unwrap(), s(&[r"C:\Users\me"]));
        assert_eq!(split_args("--name=ÆØÅ 日本").unwrap(), s(&["--name=ÆØÅ", "日本"]));
    }

    #[test]
    fn broken_arguments_are_errors() {
        assert!(split_args("'open").unwrap_err().contains("isn't closed"));
        assert!(split_args("\"open").unwrap_err().contains("isn't closed"));
        assert!(split_args("trailing\\").is_err());
        assert!(split_args("a\u{7}b").is_err());
        assert!(split_args(&"a ".repeat(MAX_ARGS + 1)).is_err());
        assert!(split_args(&"a".repeat(MAX_TEXT + 1)).is_err());
        assert_eq!(split_args(&"a ".repeat(MAX_ARGS)).unwrap().len(), MAX_ARGS);
    }

    #[test]
    fn variables_are_name_equals_value_lines() {
        let text = "# comment\nRUST_LOG=debug\n\n  PHOTOCRAFT_GPU = wgpu\nEMPTY=\nURL=https://x/?a=b\r\nRUST_LOG=info\n";
        assert_eq!(
            parse_env(text).unwrap(),
            vec![
                ("PHOTOCRAFT_GPU".into(), " wgpu".into()),
                ("EMPTY".into(), String::new()),
                ("URL".into(), "https://x/?a=b".into()),
                ("RUST_LOG".into(), "info".into())
            ]
        );
        assert!(parse_env("NOVALUE").unwrap_err().contains("line 1"));
        assert!(parse_env("ok=1\n1BAD=x").unwrap_err().contains("line 2"));
        assert!(parse_env("A B=x").is_err());
        assert!(parse_env("=x").is_err());
        assert!(parse_env("A=\u{1b}[31m").is_err());
        let many: String = (0..=MAX_VARS).map(|i| format!("V{i}=1\n")).collect();
        assert!(parse_env(&many).is_err());
    }

    #[test]
    fn a_profile_parses_both_parts_and_knows_when_it_is_empty() {
        assert!(LaunchProfile::default().is_empty());
        assert!(LaunchProfile { args: "  ".into(), env: "# nothing\n".into() }.is_empty());
        let p = LaunchProfile { args: "--safe-mode".into(), env: "A=1".into() };
        assert!(!p.is_empty());
        assert_eq!(p.parse().unwrap(), LaunchOptions { args: s(&["--safe-mode"]), env: vec![("A".into(), "1".into())] });
        assert!(LaunchProfile { args: "'".into(), env: String::new() }.parse().is_err());
        let saved: LaunchProfile = serde_json::from_str(r#"{"args":"-v"}"#).unwrap();
        assert_eq!(saved, LaunchProfile { args: "-v".into(), env: String::new() });
    }
}
