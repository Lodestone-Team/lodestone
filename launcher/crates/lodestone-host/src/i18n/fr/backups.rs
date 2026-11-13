use crate::i18n::{BackupsMsg as M, BackupsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Lead => {
            "Chaque sauvegarde est une copie complète du serveur, mais les fichiers inchangés ne sont stockés qu’une fois : sauvegarder souvent prend donc peu de place."
        }
        M::BackUpNow => "Sauvegarder maintenant",
        M::BackingUp => "Sauvegarde…",
        M::Restoring => "Restauration…",
        M::JobRunning => "Une sauvegarde ou une restauration de ce serveur est en cours.",
        M::NoSettingsPermission => {
            "Vous devez avoir l’autorisation de modifier les paramètres de ce serveur."
        }
        M::NoWritePermission => {
            "Vous devez avoir l’autorisation de modifier les fichiers de ce serveur."
        }
        M::NoSchedulePermission => {
            "Vous devez avoir l’autorisation de modifier les paramètres et les fichiers de ce serveur."
        }
        M::LoadFailed => "Impossible de charger les sauvegardes : {error}",
        M::Schedule => "Planification",
        M::Automatic => "Sauvegarder automatiquement",
        M::AutomaticHint => {
            "Rien n’est enregistré si rien n’a changé depuis la dernière sauvegarde."
        }
        M::NextBackupIn => "Prochaine sauvegarde dans {time}",
        M::NextBackupSoon => "Prochaine sauvegarde sous peu",
        M::NextBackupWhenPlayerJoins => "Prochaine sauvegarde dès qu’un joueur se connecte",
        M::Interval => "Fréquence",
        M::EveryHour => "Toutes les heures",
        M::EveryDay => "Tous les jours",
        M::OnlyWhenActive => "Seulement si des joueurs se sont connectés",
        M::OnlyWhenActiveHint => "Saute la sauvegarde si personne n’a joué depuis la précédente.",
        M::Keep => "Conserver",
        M::KeepHint => {
            "Le nombre de sauvegardes automatiques conservées. Les sauvegardes manuelles et épinglées sont toujours conservées."
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
            "Aucune sauvegarde pour l’instant. Sauvegardez maintenant, ou activez les sauvegardes automatiques."
        }
        M::DateAt => "{date}, {time}",
        M::SizeNew => "{size} · {new} nouveaux",
        M::TagManual => "Manuelle",
        M::TagAutomatic => "Automatique",
        M::TagBeforeRestore => "Avant restauration",
        M::TagLive => "À chaud",
        M::TagLiveHint => "Faite pendant que le serveur tournait.",
        M::TagInconsistent => "Peut-être incohérente",
        M::TagInconsistentHint => {
            "Le serveur n’a pas confirmé avoir enregistré le monde avant la copie : le monde a pu être copié en pleine sauvegarde."
        }
        M::TagSkippedHint => {
            "Les fichiers illisibles, par exemple ceux en cours d’utilisation, ont été laissés de côté ou repris de la sauvegarde précédente."
        }
        M::TagPinned => "Épinglée",
        M::TagPinnedHint => "Le nettoyage automatique ne la supprime jamais.",
        M::RestoreAction => "Restaurer…",
        M::DownloadLabel => "Télécharger en zip",
        M::PinLabel => "Épingler, pour qu’elle ne soit jamais supprimée automatiquement",
        M::UnpinLabel => "Désépingler",
        M::RestoreAsNewLabel => "Restaurer en tant que nouveau serveur",
        M::DeleteLabel => "Supprimer la sauvegarde",
        M::RestoreTitle => "Restaurer « {name} »",
        M::ScopeEverything => "Tout",
        M::ScopeWorld => "Seulement le monde",
        M::ScopeEverythingHint => {
            "Remplace les fichiers du serveur par ceux de la sauvegarde. Son nom, son port et ses paramètres Java restent."
        }
        M::ScopeWorldHint => {
            "Remplace le monde, son Nether et son End. Les paramètres, mods et plugins ne changent pas."
        }
        M::NoWorld => {
            "Cette sauvegarde n’a pas de dossier de monde : seul le serveur entier peut être restauré."
        }
        M::StopFirst => "Arrêter le serveur d’abord",
        M::StartAfter => "Le redémarrer ensuite",
        M::MustStop => "Le serveur doit être arrêté pour restaurer une sauvegarde.",
        M::SafetyNote => {
            "Une sauvegarde du serveur dans son état actuel est faite d’abord, pour que vous puissiez annuler."
        }
        M::Restore => "Restaurer",
        M::RestoreStarted => "Restauration de « {name} »…",
        M::DeleteTitle => "Supprimer cette sauvegarde ?",
        M::DeleteBody => "« {name} » sera supprimée. Le serveur lui-même ne change pas.",
        M::Deleted => "Sauvegarde supprimée",
        M::NewTitle => "Restaurer en tant que nouveau serveur",
        M::NewBody => {
            "Crée un nouveau serveur à partir de cette sauvegarde, avec son propre port. Ce serveur-ci ne change pas."
        }
        M::NewName => "Nom",
        M::NewDefaultName => "{name} (restauré)",
        M::NewCreate => "Créer le serveur",
        M::NewStarted => "Création de « {name} »…",
        M::Zipping => "Compression de la sauvegarde pour le téléchargement…",
        M::Downloaded => "{name} enregistré",
        M::DownloadFailed => "Impossible de télécharger la sauvegarde : {error}",
        M::BackupDone => "Sauvegarde terminée",
        M::BackupFailed => "Échec de la sauvegarde",
        M::RestoreDone => "Restauration terminée",
        M::RestoreFailed => "Échec de la restauration",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Count => ["{count} sauvegarde", "{count} sauvegardes"],
        P::Skipped => ["{count} fichier ignoré", "{count} fichiers ignorés"],
        P::EveryMinutes => ["Toutes les {count} minute", "Toutes les {count} minutes"],
        P::EveryHours => ["Toutes les {count} heure", "Toutes les {count} heures"],
        P::EveryDays => ["Tous les {count} jour", "Tous les {count} jours"],
    }
}
