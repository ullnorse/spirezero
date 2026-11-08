//! The command-line flags of `sts2-diff`'s commands: `--flag value` pairs and presence flags.
//!
//! Strict on purpose: a flag the command does not take, a flag given twice, a value-taking flag
//! without its value and a stray positional argument are all errors. A misspelt `--ascention 10`
//! used to be ignored silently, so the run played ascension 0 and its diff "passed".

use std::collections::{BTreeMap, BTreeSet};

use crate::Result;

/// The flags one command was given.
#[derive(Debug, Default)]
pub struct Args {
    values: BTreeMap<String, String>,
    present: BTreeSet<String>,
}

impl Args {
    /// Parses `raw` for a command that takes the `--flag value` flags `valued` and the presence
    /// flags `presence`.
    ///
    /// # Errors
    /// On an unknown or repeated flag, a valued flag without a value, or a positional argument.
    pub fn parse(raw: &[String], valued: &[&str], presence: &[&str]) -> Result<Self> {
        let mut args = Self::default();
        let mut raw = raw.iter();
        while let Some(flag) = raw.next() {
            let repeated = args.values.contains_key(flag) || args.present.contains(flag);
            if repeated {
                return Err(format!("{flag} given twice").into());
            }
            if valued.contains(&flag.as_str()) {
                let value = raw.next().ok_or_else(|| format!("{flag} needs a value"))?;
                args.values.insert(flag.clone(), value.clone());
            } else if presence.contains(&flag.as_str()) {
                args.present.insert(flag.clone());
            } else if flag.starts_with("--") {
                return Err(format!("unknown flag {flag}").into());
            } else {
                return Err(format!("unexpected argument {flag}").into());
            }
        }
        Ok(args)
    }

    /// The value of `flag`.
    ///
    /// # Errors
    /// If it was not given.
    pub fn get(&self, flag: &str) -> Result<&str> {
        self.get_opt(flag)
            .ok_or_else(|| format!("missing {flag}").into())
    }

    /// The value of `flag`, if given.
    #[must_use]
    pub fn get_opt(&self, flag: &str) -> Option<&str> {
        self.values.get(flag).map(String::as_str)
    }

    /// The value of `flag` parsed, or `default` without it.
    ///
    /// # Errors
    /// If the value does not parse.
    pub fn parse_or<T: std::str::FromStr>(&self, flag: &str, default: T) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        self.get_opt(flag).map_or(Ok(default), |v| {
            v.parse().map_err(|e| format!("{flag} {v}: {e}").into())
        })
    }

    /// A `--flag 0|1` switch: `false` without it.
    ///
    /// # Errors
    /// If the value is not `0` or `1` (`--real-game 0` used to turn real-game mode on).
    pub fn switch(&self, flag: &str) -> Result<bool> {
        match self.get_opt(flag) {
            None | Some("0") => Ok(false),
            Some("1") => Ok(true),
            Some(other) => Err(format!("{flag} takes 0 or 1, not {other}").into()),
        }
    }

    /// Whether the presence flag `flag` was given.
    #[must_use]
    pub fn has(&self, flag: &str) -> bool {
        self.present.contains(flag)
    }
}

#[cfg(test)]
mod tests {
    use super::Args;

    fn parse(raw: &[&str]) -> crate::Result<Args> {
        let raw: Vec<String> = raw.iter().map(|&s| s.to_owned()).collect();
        Args::parse(&raw, &["--seed", "--real-game"], &["--all"])
    }

    #[test]
    fn flags_parse() {
        let args = parse(&["--seed", "S", "--all"]).expect("parses");
        assert_eq!(args.get("--seed").ok(), Some("S"));
        assert!(args.has("--all"));
        assert!(args.get("--real-game").is_err());
        assert!(!args.switch("--real-game").expect("absent"));
    }

    #[test]
    fn anything_unexpected_is_an_error() {
        for raw in [
            &["--ascention", "10"][..],
            &["--seed"],
            &["--seed", "A", "--seed", "B"],
            &["--all", "--all"],
            &["stray"],
        ] {
            assert!(parse(raw).is_err(), "{raw:?}");
        }
        let args = parse(&["--real-game", "0"]).expect("parses");
        assert!(!args.switch("--real-game").expect("0 is off"));
        let args = parse(&["--real-game", "yes"]).expect("parses");
        assert!(args.switch("--real-game").is_err());
    }
}
