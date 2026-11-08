use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "Quitter {app}",
        M::Language => "Langue",
        M::LanguageHint => {
            "La langue d'affichage de Lodestone Launcher. Système utilise la première de vos langues préférées qui est disponible, sinon l'anglais."
        }
        M::LanguageSystem => "Système",
        M::LanguageSystemIs => "Système ({language})",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
