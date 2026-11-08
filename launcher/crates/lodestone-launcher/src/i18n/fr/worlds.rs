use crate::i18n::{WorldsMsg as M, WorldsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {}
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
