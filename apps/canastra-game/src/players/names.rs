//! Character name rules each server sets: a pattern the whole name must match and words it may not contain.

use canastra_protocol::game::CreationFailure;
use regex::Regex;

use crate::config::Result;

pub(crate) struct NameRules {
    pattern: Regex,
    /// Lower case.
    forbidden: Vec<String>,
}

impl NameRules {
    /// Rules from a `pattern` the whole name must match and `forbidden` words, compared ignoring case.
    pub(crate) fn new(pattern: &str, forbidden: &[String]) -> Result<Self> {
        let pattern =
            Regex::new(&format!("^(?:{pattern})$")).map_err(|error| format!("characters.name_pattern: {error}"))?;
        Ok(Self { pattern, forbidden: forbidden.iter().map(|word| word.to_lowercase()).collect() })
    }

    pub(crate) fn check(&self, name: &str) -> std::result::Result<(), CreationFailure> {
        if !self.pattern.is_match(name) {
            return Err(CreationFailure::InvalidName);
        }
        let lower = name.to_lowercase();
        if self.forbidden.iter().any(|word| lower.contains(word.as_str())) {
            return Err(CreationFailure::ForbiddenName);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_whole_pattern_and_avoid_forbidden_words() {
        let rules = NameRules::new("[A-Za-z0-9]{2,16}", &["GM".to_owned()]).unwrap();
        assert_eq!(rules.check("Ana2"), Ok(()));
        assert_eq!(rules.check("A"), Err(CreationFailure::InvalidName));
        assert_eq!(rules.check("Ana Maria"), Err(CreationFailure::InvalidName));
        assert_eq!(rules.check("TheGmHero"), Err(CreationFailure::ForbiddenName));
        assert!(NameRules::new("[", &[]).is_err());
    }
}
