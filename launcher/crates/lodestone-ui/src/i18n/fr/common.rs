use crate::i18n::{CommonMsg as M, CommonPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Add => "Ajouter",
        M::Apply => "Appliquer",
        M::Back => "Retour",
        M::Browse => "Parcourir…",
        M::Cancel => "Annuler",
        M::Change => "Modifier…",
        M::Close => "Fermer",
        M::Continue => "Continuer",
        M::Copy => "Copier",
        M::Copied => "Copié",
        M::Default => "Par défaut",
        M::Delete => "Supprimer",
        M::Discard => "Abandonner",
        M::Done => "Terminé",
        M::Edit => "Modifier",
        M::Install => "Installer",
        M::Loading => "Chargement…",
        M::Next => "Suivant",
        M::No => "Non",
        M::None => "Aucun",
        M::NotNow => "Plus tard",
        M::Off => "Désactivé",
        M::Ok => "OK",
        M::On => "Activé",
        M::Open => "Ouvrir",
        M::OpenFolder => "Ouvrir le dossier",
        M::Previous => "Précédent",
        M::Refresh => "Actualiser",
        M::Remove => "Retirer",
        M::Rename => "Renommer",
        M::Reset => "Réinitialiser",
        M::Save => "Enregistrer",
        M::Saved => "Enregistré",
        M::Search => "Rechercher",
        M::SignIn => "Se connecter",
        M::SignOut => "Se déconnecter",
        M::Start => "Démarrer",
        M::Stop => "Arrêter",
        M::TryAgain => "Réessayer",
        M::Update => "Mettre à jour",
        M::Menu => "Menu",
        M::MoreActions => "Plus d’actions",
        M::MoveDown => "Descendre",
        M::MoveUp => "Monter",
        M::Yes => "Oui",
        M::MinimizeWindow => "Réduire",
        M::MaximizeWindow => "Agrandir",
        M::RestoreWindow => "Restaurer",
        M::KeepEditing => "Continuer à modifier",
        M::UnsavedChanges => "Vous avez des modifications non enregistrées",
        M::ShowInFileManager => {
            if cfg!(target_os = "macos") {
                "Afficher dans le Finder"
            } else if cfg!(windows) {
                "Afficher dans l’Explorateur de fichiers"
            } else {
                "Afficher dans le dossier"
            }
        }
        M::MoveToTrash => "Placer dans la Corbeille",
        M::TheTrash => "la Corbeille",
        M::OpenImage => {
            if cfg!(target_os = "macos") {
                "Ouvrir dans Aperçu"
            } else {
                "Ouvrir"
            }
        }
        M::ThisComputer => {
            if cfg!(target_os = "macos") {
                "ce Mac"
            } else if cfg!(windows) {
                "ce PC"
            } else {
                "cet ordinateur"
            }
        }
        M::ThisComputerStart => {
            if cfg!(target_os = "macos") {
                "Ce Mac"
            } else if cfg!(windows) {
                "Ce PC"
            } else {
                "Cet ordinateur"
            }
        }
        M::Keychain => {
            if cfg!(target_os = "macos") {
                "trousseau"
            } else if cfg!(windows) {
                "Gestionnaire d’identification"
            } else {
                "trousseau de clés"
            }
        }
        M::KeychainInFull => {
            if cfg!(target_os = "macos") {
                "trousseau macOS"
            } else if cfg!(windows) {
                "Gestionnaire d’identification Windows"
            } else {
                "trousseau de clés du système"
            }
        }
        M::FileDialogUnavailable => {
            if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                "La boîte de dialogue de fichiers du système n’est pas disponible. Elle a besoin de xdg-desktop-portal et du portail de votre bureau, comme xdg-desktop-portal-kde ou xdg-desktop-portal-gnome."
            } else {
                "La boîte de dialogue de fichiers du système n’a pas pu s’ouvrir. Réessayez."
            }
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Instances => ["{count} instance", "{count} instances"],
        P::Files => ["{count} fichier", "{count} fichiers"],
    }
}
