use crate::i18n::{CoreMsg as M, CorePlural as P};

pub fn text(message: M) -> &'static str {
    match message {}
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
