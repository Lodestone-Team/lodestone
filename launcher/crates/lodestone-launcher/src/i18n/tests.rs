//! Every locale has every message (the tables' `match`es make that a compile
//! error), and these tests check what the compiler can't: the placeholders match the
//! English, and nothing is left empty or untranslated.

use super::*;

const TRANSLATED: [Locale; 3] = [Locale::Fr, Locale::ZhHans, Locale::ZhHant];

#[test]
fn every_translation_has_the_english_placeholders() {
    let mut problems = Vec::new();
    for message in Msg::all() {
        let english = message.text(Locale::En);
        let expected = placeholders(english);
        for locale in TRANSLATED {
            let text = message.text(locale);
            if placeholders(text) != expected {
                problems.push(format!(
                    "{} in {}: {:?} has {:?}, the English {:?}",
                    message.name(),
                    locale.code(),
                    text,
                    placeholders(text),
                    expected
                ));
            }
        }
    }
    for message in Plural::all() {
        let [one, other] = message.forms(Locale::En);
        let expected = placeholders(other);
        if placeholders(one) != expected {
            problems.push(format!("{}: the English forms differ", message.name()));
        }
        if !expected.contains("count") {
            problems.push(format!("{} has no {{count}}", message.name()));
        }
        for locale in TRANSLATED {
            for text in message.forms(locale) {
                if placeholders(text) != expected {
                    problems.push(format!(
                        "{} in {}: {:?} has {:?}, the English {:?}",
                        message.name(),
                        locale.code(),
                        text,
                        placeholders(text),
                        expected
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn nothing_is_empty() {
    for locale in Locale::ALL {
        for message in Msg::all() {
            assert!(
                !message.text(locale).is_empty(),
                "{} is empty in {}",
                message.name(),
                locale.code()
            );
        }
        for message in Plural::all() {
            for text in message.forms(locale) {
                assert!(
                    !text.is_empty(),
                    "{} is empty in {}",
                    message.name(),
                    locale.code()
                );
            }
        }
    }
}

/// A Chinese message with no Chinese in it, where the English has words, is most
/// likely English that was never translated. Messages that are only product names,
/// units or punctuation are allowed.
#[test]
fn chinese_messages_are_translated() {
    let has_cjk = |text: &str| {
        text.chars().any(|c| {
            ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c)
        })
    };
    // An English word of four or more lowercase letters, outside placeholders.
    let has_words = |text: &str| {
        let mut plain = String::new();
        let mut depth = 0;
        for c in text.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ if depth == 0 => plain.push(c),
                _ => {}
            }
        }
        plain
            .split(|c: char| !c.is_ascii_alphabetic())
            .any(|word| word.len() >= 4 && word.chars().all(|c| c.is_ascii_lowercase()))
    };
    let mut problems = Vec::new();
    for locale in [Locale::ZhHans, Locale::ZhHant] {
        for message in Msg::all() {
            let text = message.text(locale);
            if has_words(message.text(Locale::En)) && !has_cjk(text) {
                problems.push(format!("{} in {}: {text:?}", message.name(), locale.code()));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "untranslated:\n{}",
        problems.join("\n")
    );
}
