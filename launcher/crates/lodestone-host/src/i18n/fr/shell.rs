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
        M::Username => "Nom d’utilisateur",
        M::Password => "Mot de passe",
        M::NoUsername => "Saisissez un nom d’utilisateur.",
        M::NoPassword => "Saisissez un mot de passe.",
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
        M::TabEventLogs => "Journal des événements",
        M::Settings => "Paramètres",
        M::SettingsMenu => "Paramètres…",
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
        M::PlayersOnline => "{online}/{max} en ligne",
        M::PowerMenu => "Alimentation : {state}",
        M::Connected => "Connecté",
        M::Connecting => "Connexion…",
        M::Reconnecting => "Reconnexion…",
        M::SignedOut => "Déconnecté",
        M::SignedInAs => "Connecté en tant que {name}",
        M::HomeTitle => "Bienvenue dans Lodestone Host",
        M::HomeSubtitle => "Sélectionnez un serveur dans la barre latérale, ou créez-en un.",
        M::CreateServer => "Créer un serveur",
        M::NotBuilt => "Cette page n’est pas encore dans Lodestone Host.",
        M::InstanceNotFound => "Serveur introuvable",
        M::ShowHost => "Afficher Lodestone Host",
        M::QuitHost => "Quitter Lodestone Host",
        M::DevBadge => "DEV",
        M::CoreVersion => "Version {version}",
        M::Degraded => "Mises à jour en direct interrompues",
        M::DegradedBanner => {
            "Lodestone Host joint le core, mais les mises à jour en direct se sont arrêtées. Ce qui s’affiche n’est peut-être plus à jour."
        }
        M::Reload => "Recharger",
        M::CoreAddress => "Adresse : {address}",
        M::CoreHost => "Ordinateur : {host}",
        M::CoreSystem => "Système : {system}",
        M::CoreUptime => "En marche depuis {uptime}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
