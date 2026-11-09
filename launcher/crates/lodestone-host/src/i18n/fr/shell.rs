use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::StartingCore => "Démarrage de Lodestone Core…",
        M::RestartingCore => "Lodestone Core s’est arrêté. Redémarrage…",
        M::CoreFailed => "Lodestone Core n’a pas pu démarrer",
        M::CoreLog => "La fin de son journal :",
        M::OpenLogFolder => "Ouvrir le dossier du journal",
        M::StoppingCore => "Arrêt de vos serveurs…",
        M::SigningIn => "Connexion…",
        M::SetupTitle => "Configurer Lodestone Core",
        M::SetupIntro => {
            "Créez le compte propriétaire. Vous vous connectez avec lui depuis le tableau de bord web et vos autres appareils."
        }
        M::Username => "Nom d’utilisateur",
        M::Password => "Mot de passe",
        M::ConfirmPassword => "Confirmer le mot de passe",
        M::CreateOwner => "Créer le compte propriétaire",
        M::NoUsername => "Saisissez un nom d’utilisateur.",
        M::NoPassword => "Saisissez un mot de passe.",
        M::PasswordsDiffer => "Les mots de passe ne correspondent pas.",
        M::FirstConfigTitle => "Nommez votre core",
        M::CoreName => "Nom du core",
        M::PublicAddress => "Domaine ou adresse IP publique",
        M::PublicAddressHint => {
            "Les joueurs rejoignent vos serveurs avec cette adresse. Vous pouvez la laisser vide."
        }
        M::SignInTitle => "Se connecter",
        M::SignInIntro => {
            "Ce core ne laisse pas les applications de cet ordinateur obtenir l’accès propriétaire. Connectez-vous avec votre compte Lodestone."
        }
        M::SelectedInstance => "Instance sélectionnée",
        M::AllInstances => "Toutes les instances",
        M::NoInstanceSelected => "Aucune instance sélectionnée",
        M::NoInstances => "Aucune instance pour l’instant",
        M::NewServer => "Nouveau serveur",
        M::NoPermission => "Pas d’autorisation",
        M::SettingUp => "Préparation de {name}…",
        M::TabOverview => "Vue d’ensemble",
        M::TabSettings => "Paramètres",
        M::TabConsole => "Console",
        M::TabFiles => "Fichiers",
        M::TabMods => "Mods",
        M::TabPlugins => "Plugins",
        M::TabMacros => "Macros",
        M::TabEventLogs => "Journal des événements",
        M::TabPlayit => "Playit.gg",
        M::Settings => "Paramètres",
        M::SettingsCore => "Core",
        M::SettingsGeneral => "Général",
        M::SettingsUsers => "Utilisateurs",
        M::SettingsAccount => "Compte",
        M::SettingsProfile => "Profil",
        M::StateStarting => "Démarrage",
        M::StateRunning => "En marche",
        M::StateStopping => "Arrêt",
        M::StateStopped => "Arrêté",
        M::StateError => "Planté",
        M::StateUnknown => "Inconnu",
        M::Restart => "Redémarrer",
        M::Kill => "Forcer l’arrêt",
        M::PlayersOnline => "{online}/{max} joueurs",
        M::Connected => "Connecté",
        M::Connecting => "Connexion…",
        M::Reconnecting => "Reconnexion…",
        M::SignedOut => "Déconnecté",
        M::SignedInAs => "Connecté en tant que {name}",
        M::HomeTitle => "Bienvenue dans Lodestone Host",
        M::HomeSubtitle => "Sélectionnez un serveur dans la barre latérale, ou créez-en un.",
        M::CreateServer => "Créer un serveur",
        M::NotBuilt => "Cette page n’est pas encore dans Lodestone Host.",
        M::NeedsScripting => {
            "Les macros nécessitent un core avec les scripts. Le core de Lodestone Host fonctionne sans."
        }
        M::InstanceNotFound => "Serveur introuvable",
        M::ShowHost => "Afficher Lodestone Host",
        M::QuitHost => "Quitter Lodestone Host",
        M::DevBadge => "DEV",
        M::CoreVersion => "Version {version}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
