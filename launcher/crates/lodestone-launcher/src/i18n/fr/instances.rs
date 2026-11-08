use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {}
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
