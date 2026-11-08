use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "結束 {app}",
        M::Language => "語言",
        M::LanguageHint => {
            "Lodestone Launcher 的顯示語言。「系統」會使用你偏好語言中第一個可用的語言，否則使用英文。"
        }
        M::LanguageSystem => "系統",
        M::LanguageSystemIs => "系統（{language}）",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
