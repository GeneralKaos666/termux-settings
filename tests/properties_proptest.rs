//! Property tests for the line-preserving `*.properties` editor.
//!
//! The core promise is that editing one key never disturbs any other line, so
//! these drive arbitrary comment, continuation and unknown-key lines through
//! `set_value` and check every one survives verbatim.

use proptest::prelude::*;
use termux_settings::persist::properties::{parse_active, set_value, unset_value};

/// Printable, single-line text that cannot collide with the key under test.
fn unrelated_line() -> impl Strategy<Value = String> {
    "[ -~]{0,60}".prop_filter("the key under test is not in this line", |line| {
        !line.contains("setting")
    })
}

fn document() -> impl Strategy<Value = String> {
    proptest::collection::vec(unrelated_line(), 0..20).prop_map(|lines| {
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    })
}

/// A value as the editors pass one: no surrounding whitespace, because readers
/// trim values.
fn trimmed_value() -> impl Strategy<Value = String> {
    "[!-~]([ -~]{0,18}[!-~])?"
}

proptest! {
    #[test]
    fn setting_one_key_preserves_every_other_line(
        text in document(),
        value in trimmed_value(),
    ) {
        let output = set_value(&text, "setting", &value);

        for line in text.lines() {
            prop_assert!(
                output.lines().any(|candidate| candidate == line),
                "line {line:?} disappeared from {output:?}"
            );
        }
        let pairs = parse_active(&output);
        let stored = pairs.iter().find(|(key, _)| key == "setting");
        prop_assert_eq!(
            stored.map(|(_, value)| value.as_str()),
            Some(value.trim())
        );
    }

    #[test]
    fn setting_the_same_key_twice_is_idempotent(
        text in document(),
        value in trimmed_value(),
    ) {
        let once = set_value(&text, "setting", &value);
        let twice = set_value(&once, "setting", &value);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn unsetting_keeps_the_line_count(
        text in document(),
        value in trimmed_value(),
    ) {
        let set = set_value(&text, "setting", &value);
        let unset = unset_value(&set, "setting");
        prop_assert_eq!(set.lines().count(), unset.lines().count());
        prop_assert!(
            !parse_active(&unset).iter().any(|(key, _)| key == "setting"),
            "the key is still active in {unset:?}"
        );
    }
}
