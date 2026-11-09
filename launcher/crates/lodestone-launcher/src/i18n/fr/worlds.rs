use crate::i18n::{WorldsMsg as M, WorldsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Worlds => "Mondes",
        M::Screenshots => "Captures d’écran",

        M::StopGameFirst => {
            "Arrêtez d’abord le jeu\u{a0}: il a ce monde ouvert et l’enregistre lorsqu’il se ferme."
        }
        M::WaitForTask => "Attendez la fin de la tâche en cours.",
        M::Cancelled => "Annulé.",
        M::GameRunning => {
            "Le jeu est en marche\u{a0}: vous pouvez ouvrir les mondes et voir les sauvegardes, mais les modifications attendront son arrêt."
        }

        M::BackingUp => "Sauvegarde de {name}",
        M::RestoringBackup => "Restauration de la sauvegarde",
        M::ReplacingWorld => "Remplacement du monde par la sauvegarde",
        M::MovingToTrash => "Déplacement de {name} dans {trash}",
        M::MovingBackupToTrash => "Déplacement de la sauvegarde dans {trash}",
        M::Renaming => "Renommage",
        M::Exporting => "Exportation de {name}",
        M::ImportingWorlds => "Importation des mondes",

        M::BackedUp => "{name} sauvegardé ({size}).",
        M::RestoredAsCopy => "Restauré en tant que nouveau monde, {folder}.",
        M::Restored => "{folder} restauré.",
        M::RestoredWithSafety => {
            "{folder} restauré. Le monde tel qu’il était est sauvegardé sous {backup}, et dans {trash}."
        }
        M::InTrash => "{name} est dans {trash}.",
        M::RenamedTo => "Renommé en {name}.",
        M::RenamedToFolder => "Renommé en {name} (dossier {folder}).",
        M::Exported => "{name} exporté vers {path} ({size}).",
        M::Imported => "Importé\u{a0}: {worlds}.",
        M::FileError => "{name}\u{a0}: {error}",
        M::ZipsOnly => "Les mondes s’importent depuis des fichiers .zip.",
        M::ImportWorldPrompt => "Importer le monde",
        M::CopiedScreenshot => "Capture d’écran copiée.",
        M::CouldntCopy => "Impossible de la copier\u{a0}: {error}",

        M::Hardcore => "Hardcore",
        M::Survival => "Survie",
        M::Creative => "Créatif",
        M::Adventure => "Aventure",
        M::Spectator => "Spectateur",
        M::Cheats => "Commandes",
        M::Snapshot => "{version} (snapshot)",
        M::PlayedAt => "Joué\u{a0}: {time}",
        M::BackUp => "Sauvegarder",
        M::Backups => "Sauvegardes",
        M::Export => "Exporter…",
        M::DeleteWorld => "Supprimer…",
        M::NoBackupsYet => {
            "Aucune sauvegarde pour l’instant. «\u{a0}Sauvegarder\u{a0}» enregistre le monde dans un zip, dans le dossier backups de l’instance."
        }
        M::RestoreAsCopy => "Restaurer une copie",
        M::ReplaceWorld => "Remplacer le monde…",

        M::ImportWorld => "Importer un monde…",
        M::OpenSavesFolder => "Ouvrir le dossier saves",
        M::ReadingWorlds => "Lecture des mondes",
        M::NoWorldsYet => "Aucun monde pour l’instant",
        M::NoWorldsHint => {
            "Les mondes que vous créez dans le jeu apparaissent ici. Vous pouvez aussi importer un monde en zip, ou le déposer ici."
        }

        M::ReadingScreenshots => "Lecture des captures d’écran",
        M::NoScreenshotsYet => "Aucune capture d’écran pour l’instant",
        M::NoScreenshotsHint => {
            "Appuyez sur F2 dans le jeu pour en prendre une\u{202F}; elles apparaissent ici."
        }
        M::Position => "{index} sur {count}",
        M::CopyImage => "Copier l’image",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScreenshotsSummary => [
            "{count} capture d’écran · F2 dans le jeu en prend une",
            "{count} captures d’écran · F2 dans le jeu en prend une",
        ],
    }
}
