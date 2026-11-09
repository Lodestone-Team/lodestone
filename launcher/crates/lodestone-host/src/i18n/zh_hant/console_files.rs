use crate::i18n::{ConsoleFilesMsg as M, ConsoleFilesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
