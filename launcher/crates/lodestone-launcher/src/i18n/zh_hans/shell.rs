use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "退出 {app}",
        M::Language => "语言",
        M::LanguageHint => {
            "Lodestone Launcher 的显示语言。“系统”会使用你的首选语言中第一个可用的语言，否则使用英语。"
        }
        M::LanguageSystem => "系统",
        M::LanguageSystemIs => "系统（{language}）",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
