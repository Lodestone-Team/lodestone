use crate::i18n::{ValueMsg as M, ValuePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KeyValue => "{key}\u{a0}: {value}",
        M::FromTo => "Passe de {from} à {to}",
        M::NeededOfFree => "{needed} nécessaires, {free} libres",
        M::Online => "En ligne",
        M::Offline => "Hors ligne",
        M::Running => "En marche",
        M::Stopped => "Arrêté",
        M::Starting => "Démarrage",
        M::Stopping => "Arrêt en cours",
        M::Failed => "Échec",
        M::Installing => "Installation",
        M::Downloading => "Téléchargement",
        M::Copying => "Copie",
        M::Paused => "En pause",
        M::KindFolder => "Dossier",
        M::KindWorld => "Monde",
        M::KindMod => "Mod",
        M::KindResourcePack => "Pack de ressources",
        M::KindShaderPack => "Pack de shaders",
        M::KindDataPack => "Pack de données",
        M::KindPlugin => "Plugin",
        M::KindModpack => "Modpack",
        M::KindImage => "Image",
        M::KindServer => "Serveur",
        M::KindInstance => "Instance",
        M::KindArchive => "Archive",
        M::KindFile => "Fichier",
        M::KindBackup => "Sauvegarde",
        M::KindSnapshot => "Instantané",
        M::KindFriend => "Ami",
        M::KindComputer => "Ordinateur",
        M::UntitledSnapshot => "Instantané sans titre",
        M::UntitledBackup => "Sauvegarde sans titre",
        M::BeforeUpdate => "Avant la mise à jour",
        M::BeforeRestore => "Avant la restauration",
        M::CopyOf => "Copie de",
        M::Automatic => "Automatique",
        M::Added => "Ajoutés",
        M::Skipped => "Ignorés",
        M::NotAdded => "Non ajoutés",
        M::Reveal => "Afficher",
        M::Hide => "Masquer",
        M::Hidden => "Masqué",
        M::OpenLink => "Ouvrir le lien",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShowMore => ["Afficher {count} de plus", "Afficher {count} de plus"],
    }
}
