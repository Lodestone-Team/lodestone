use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "Chaque instantané est une copie complète du dossier du serveur, mais les fichiers inchangés ne sont stockés qu’une fois : en prendre souvent prend donc peu de place."
        }
        M::BackUpNow => "Prendre un instantané",
        M::BackingUp => "Prise d’un instantané…",
        M::Restoring => "Restauration…",
        M::JobRunning => {
            "Un instantané, une sauvegarde du monde ou une restauration de ce serveur est en cours."
        }
        M::NoSettingsPermission => {
            "Vous devez avoir l’autorisation de modifier les paramètres de ce serveur."
        }
        M::NoWritePermission => {
            "Vous devez avoir l’autorisation de modifier les fichiers de ce serveur."
        }
        M::NoSchedulePermission => {
            "Vous devez avoir l’autorisation de modifier les paramètres et les fichiers de ce serveur."
        }
        M::LoadFailed => "Impossible de charger les instantanés : {error}",
        M::Schedule => "Planification",
        M::Automatic => "Prendre des instantanés automatiquement",
        M::AutomaticHint => {
            "Rien n’est enregistré si rien n’a changé depuis le dernier instantané."
        }
        M::NextBackupIn => "Prochain instantané dans {time}",
        M::NextBackupSoon => "Prochain instantané sous peu",
        M::NextBackupWhenPlayerJoins => "Prochain instantané dès qu’un joueur se connecte",
        M::Interval => "Fréquence",
        M::EveryHour => "Toutes les heures",
        M::EveryDay => "Tous les jours",
        M::OnlyWhenActive => "Seulement si des joueurs se sont connectés",
        M::OnlyWhenActiveHint => "Depuis le dernier instantané.",
        M::Keep => "Conserver",
        M::KeepHint => {
            "Instantanés automatiques uniquement. Les manuels et les épinglés sont toujours conservés."
        }
        M::KeepFewer => "Moins",
        M::KeepNormal => "Normal",
        M::KeepMore => "Plus",
        M::KeepCustom => "Personnalisé",
        M::ScheduleSaveFailed => "Impossible d’enregistrer la planification : {error}",
        M::StorageFiles => "{size} de fichiers",
        M::StorageStored => "{size} stockés",
        M::StorageFree => "{size} libres",
        M::Empty => {
            "Aucun instantané pour l’instant. Prenez-en un maintenant, ou activez les instantanés automatiques."
        }
        M::DateAt => "{date}, {time}",
        M::SizeNew => "{size} · {new} nouveaux",
        M::TagManual => "À la main",
        M::TagAutomatic => "Automatique",
        M::TagBeforeRestore => "Avant restauration",
        M::TagLive => "À chaud",
        M::TagLiveHint => "Pendant que le serveur tournait.",
        M::TagInconsistent => "Cohérence incertaine",
        M::TagInconsistentHint => {
            "Le serveur n’a pas confirmé avoir enregistré le monde avant la copie : le monde a pu être copié en pleine sauvegarde."
        }
        M::TagSkippedHint => {
            "Les fichiers illisibles, par exemple ceux en cours d’utilisation, ont été laissés de côté ou repris de l’instantané précédent."
        }
        M::TagPinned => "Épinglé",
        M::TagPinnedHint => "Le nettoyage automatique ne le supprime jamais.",
        M::RestoreAction => "Restaurer…",
        M::DownloadLabel => "Télécharger en zip",
        M::PinLabel => "Épingler, pour qu’il ne soit jamais supprimé automatiquement",
        M::UnpinLabel => "Désépingler",
        M::RestoreAsNewLabel => "Restaurer en tant que nouveau serveur",
        M::DeleteLabel => "Supprimer l’instantané",
        M::RestoreTitle => "Restaurer « {name} »",
        M::ScopeEverything => "Tout",
        M::ScopeWorld => "Seulement le monde",
        M::ScopeEverythingHint => {
            "Remplace les fichiers du serveur par ceux de l’instantané. Son nom, son port et ses paramètres Java restent."
        }
        M::ScopeWorldHint => {
            "Remplace le monde, son Nether et son End. Les paramètres, mods et plugins ne changent pas."
        }
        M::NoWorld => {
            "Cet instantané n’a pas de dossier de monde : seul le serveur entier peut être restauré."
        }
        M::StopFirst => "Arrêter le serveur d’abord",
        M::StartAfter => "Le redémarrer ensuite",
        M::MustStop => "Le serveur doit être arrêté pour restaurer un instantané.",
        M::SafetyNote => {
            "Un instantané du serveur dans son état actuel est pris d’abord, pour que vous puissiez annuler."
        }
        M::Restore => "Restaurer",
        M::RestoreStarted => "Restauration de « {name} »…",
        M::DeleteTitle => "Supprimer cet instantané ?",
        M::DeleteBody => "« {name} » sera supprimé. Le serveur lui-même ne change pas.",
        M::Deleted => "Instantané supprimé",
        M::NewTitle => "Restaurer en tant que nouveau serveur",
        M::NewBody => {
            "Crée un nouveau serveur à partir de cet instantané, avec son propre port. Ce serveur-ci ne change pas."
        }
        M::NewName => "Nom",
        M::NewDefaultName => "{name} (restauré)",
        M::NewCreate => "Créer le serveur",
        M::NewStarted => "Création de « {name} »…",
        M::Zipping => "Compression de l’instantané pour le téléchargement…",
        M::Downloaded => "{name} enregistré",
        M::DownloadFailed => "Impossible de télécharger l’instantané : {error}",
        M::BackupDone => "Instantané terminé",
        M::BackupFailed => "Échec de l’instantané",
        M::RestoreDone => "Restauration terminée",
        M::RestoreFailed => "Échec de la restauration",
        M::ExportLabel => "Exporter en fichier pour un autre Host ou un Launcher",
        M::ExportTitle => "Exporter « {name} »",
        M::ExportBody => {
            "Enregistre cet instantané dans un fichier .lodeimg. Un autre Lodestone Host peut en faire un serveur, et Lodestone Launcher une instance solo de son monde. Les mots de passe de server.properties sont vidés, et les fichiers qui contiennent un mot de passe ou un jeton sont laissés de côté."
        }
        M::ExportPlugins => {
            "Inclure les paramètres des plugins (ils peuvent contenir des mots de passe)"
        }
        M::ExportPluginsHint => {
            "Tout le contenu des dossiers des plugins : paramètres, bases de données et données des joueurs."
        }
        M::ExportAction => "Exporter…",
        M::ExportPreparing => "Préparation du fichier…",
        M::ExportSaving => "Enregistrement du fichier…",
        M::Exported => "{name} enregistré",
        M::ExportedBlanked => "Vidé : {names}",
        M::ExportedNotDownloadable => {
            "Absents du fichier, leurs auteurs n’autorisant pas le téléchargement ailleurs : {names}"
        }
        M::ExportedSecretsWithheld => {
            "Laissés de côté, car ils contiennent un mot de passe ou un jeton (l’autre serveur écrit ses valeurs par défaut) : {names}"
        }
        M::ExportFailed => "Impossible d’exporter l’instantané : {error}",
        M::ExportCancelled => "L’exportation a été annulée",
        M::WorldSection => "Sauvegardes du monde",
        M::SnapshotsSection => "Instantanés",
        M::WorldLead => {
            "Une sauvegarde du monde est un simple .zip du monde, de son Nether et de son End, qui s’ouvre aussi en dehors de Lodestone. Elle n’est faite que quand vous le demandez."
        }
        M::WorldBackUp => "Sauvegarder le monde…",
        M::WorldCreateTitle => "Sauvegarder le monde",
        M::WorldCreateBody => {
            "Enregistre le monde dans un .zip. Un serveur en marche continue de tourner ; l’enregistrement est mis en pause pendant la copie du monde."
        }
        M::WorldName => "Nom (facultatif)",
        M::WorldCreate => "Sauvegarder",
        M::WorldBackingUp => "Sauvegarde du monde…",
        M::WorldRestoring => "Restauration du monde…",
        M::WorldEmpty => "Aucune sauvegarde du monde pour l’instant.",
        M::WorldLoadFailed => "Impossible de charger les sauvegardes du monde : {error}",
        M::WorldSizeVersion => "{size} · Minecraft {version}",
        M::WorldBeforeRestoreHint => "Faite avant de restaurer « {name} ».",
        M::WorldTagInconsistentHint => {
            "Le serveur n’a pas confirmé avoir enregistré le monde avant la copie."
        }
        M::WorldLeftOutHint => {
            "Les liens, les fichiers spéciaux, les fichiers illisibles et les noms en conflit sur certains disques ont été laissés de côté."
        }
        M::WorldDownloadLabel => "Télécharger le .zip",
        M::WorldDeleteLabel => "Supprimer la sauvegarde du monde",
        M::WorldRestoreTitle => "Restaurer le monde depuis « {name} »",
        M::WorldRestoreBody => {
            "Remplace le monde de ce serveur, son Nether et son End par ceux de la sauvegarde. Les paramètres, mods et plugins ne changent pas."
        }
        M::WorldMustStop => "Le serveur doit être arrêté pour restaurer une sauvegarde du monde.",
        M::WorldSafetyNote => {
            "Le monde actuel est d’abord enregistré dans une sauvegarde du monde, pour que vous puissiez annuler."
        }
        M::WorldRestoreStarted => "Restauration du monde depuis « {name} »…",
        M::WorldDeleteTitle => "Supprimer cette sauvegarde du monde ?",
        M::WorldDeleteBody => "« {name} » sera supprimée. Le monde du serveur ne change pas.",
        M::WorldDeleted => "Sauvegarde du monde supprimée",
        M::WorldDownloadFailed => "Impossible de télécharger la sauvegarde du monde : {error}",
        M::WorldDownloadCancelled => "Le téléchargement a été annulé",
        M::WorldBackupDone => "Sauvegarde du monde terminée",
        M::WorldBackupFailed => "Échec de la sauvegarde du monde",
        M::WorldRestoreDone => "Monde restauré",
        M::WorldRestoreFailed => "Impossible de restaurer le monde",
        M::WorldCancelFailed => "Impossible d’annuler : {error}",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} instantané", "{count} instantanés"],
        P::Skipped => ["{count} fichier ignoré", "{count} fichiers ignorés"],
        P::EveryMinutes => ["Toutes les {count} minute", "Toutes les {count} minutes"],
        P::EveryHours => ["Toutes les {count} heure", "Toutes les {count} heures"],
        P::EveryDays => ["Tous les {count} jour", "Tous les {count} jours"],
        P::ExportedLeftOut => [
            "{count} fichier qui ne doit pas quitter cet ordinateur a été retiré",
            "{count} fichiers qui ne doivent pas quitter cet ordinateur ont été retirés",
        ],
        P::WorldCount => [
            "{count} sauvegarde du monde",
            "{count} sauvegardes du monde",
        ],
        P::WorldLeftOut => [
            "{count} fichier laissé de côté",
            "{count} fichiers laissés de côté",
        ],
    }
}
