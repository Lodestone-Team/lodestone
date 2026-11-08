//! The window's frame: top bar, sidebar, Play tab, the launcher's Settings dialog, accounts and sign-in, and the shared components.

crate::i18n::messages! {
    pub enum ShellMsg {
        /// The app menu's Quit; {app} is "Lodestone Launcher".
        QuitApp = "Quit {app}",
        /// Settings > General.
        Language = "Language",
        LanguageHint = "The language Lodestone Launcher is shown in. System uses the first of your preferred languages it has, else English.",
        /// The Language setting's first choice.
        LanguageSystem = "System",
        /// The setting's button while it is System: {language} is the language it uses.
        LanguageSystemIs = "System ({language})",
    }

    pub enum ShellPlural {
    }
}
