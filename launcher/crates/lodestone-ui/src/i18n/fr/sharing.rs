use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByOther => "Créé par un autre programme",
        M::ImageCarried => "Dans le fichier lui-même, non téléchargés depuis Modrinth",
        M::ImageClientOnly => "Mods que seuls les joueurs utilisent, laissés de côté",
        M::ImageNotDownloadable => {
            "Les serveurs Lodestone ne peuvent pas encore télécharger depuis CurseForge, donc ces fichiers ne seront pas installés."
        }
        M::ImageTakenOut => "Retirés par l’expéditeur",
        M::ImageBlanked => "Vidés par l’expéditeur",
        M::ImageSecretsNotTaken => {
            "Non repris, car ils contiennent une clé du serveur, un mot de passe ou un jeton"
        }
        M::ImageWithheld => "Absents du fichier",
        M::ImageNotTaken => "Non repris du fichier",
        M::NotTakenSoftware => "Logiciel du serveur",
        M::NotTakenEula => "CLUF",
        M::NotTakenClientFiles => "Fichiers propres aux joueurs",
        M::NotTakenOperators => "Opérateurs",
        M::NotTakenScripts => "Scripts",
        M::ImageOfflineMode => "Les joueurs peuvent rejoindre sans compte Minecraft.",
        M::ImageServerSoftware => "Logiciel serveur",
        M::ImageServerSoftwareHint => {
            "Paper exécute des plugins. Un serveur vanilla exécute Minecraft tel quel."
        }
        M::ImageWorld => "Monde",
        M::ImageWorldHint => "Seul ce monde est copié sur le serveur.",
        M::ImageNoWorld => "Aucun monde (un nouveau)",
        M::ImageDownloads => "Depuis Modrinth",
        M::ImageDisk => "Espace disque",
        M::ImageNoRoom => {
            "Il n’y a pas assez d’espace libre pour créer ce serveur avec le monde du fichier. Un monde plus petit, ou aucun, peut tenir."
        }
        M::ImageNeedsGlobal => {
            "Ce fichier contient ses propres fichiers jar : créer un serveur à partir de lui nécessite l’autorisation de modifier les fichiers globaux."
        }
        M::ImageProtectedWarning => {
            "Il contient ses propres fichiers protégés, comme des fichiers jar ou des programmes, venant de la personne qui l’a créé. N’utilisez que des fichiers de personnes de confiance."
        }
        M::ImageMadeAsPaper => {
            "Lodestone crée ces serveurs en tant que serveurs Paper, qui utilisent les mêmes plugins."
        }
        M::ImageUnsupported => "Lodestone ne peut pas créer de serveurs avec ce chargeur.",
        M::ImageMadeWith => "Créé avec",
        M::ImageFiles => "Fichiers du fichier",
        M::ImageSize => "Taille du fichier",
        M::ImageBasedOn => "Basé sur",
        M::ImageNotInstalled => "Ne seront pas installés",
        M::ImageOff => "Désactivé",
        M::ImageDownloadedMods => "Téléchargés depuis Modrinth",
        M::ImageDiskSpare => "L’espace requis inclut 1 Go de marge.",
        M::NotTakenSoftwareReason => "Lodestone installe le sien",
        M::NotTakenOperatorsReason => "Peuvent être gardés",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "Il contient {count} mod ou plugin : un programme qui s’exécute sur le serveur. N’utilisez que des fichiers de personnes de confiance.",
            "Il contient {count} mods ou plugins : des programmes qui s’exécutent sur le serveur. N’utilisez que des fichiers de personnes de confiance.",
        ],
        P::OutcomeNotDownloaded => [
            "{count} fichier n’a pas pu être téléchargé et a été laissé de côté",
            "{count} fichiers n’ont pas pu être téléchargés et ont été laissés de côté",
        ],
        P::OutcomeNoSource => [
            "{count} fichier ne peut pas être téléchargé par les serveurs Lodestone et a été laissé de côté",
            "{count} fichiers ne peuvent pas être téléchargés par les serveurs Lodestone et ont été laissés de côté",
        ],
        P::OutcomeClientJars => [
            "{count} mod qui ne fonctionne que sur les ordinateurs des joueurs a été retiré",
            "{count} mods qui ne fonctionnent que sur les ordinateurs des joueurs ont été retirés",
        ],
        P::OutcomeSecrets => [
            "{count} fichier contenant une clé du serveur, un mot de passe ou un jeton a été laissé de côté : le serveur écrit ses valeurs par défaut",
            "{count} fichiers contenant une clé du serveur, un mot de passe ou un jeton ont été laissés de côté : le serveur écrit ses valeurs par défaut",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} fichier de paramètres n’a pas pu être vérifié et a été laissé de côté : le serveur écrit ses valeurs par défaut",
            "{count} fichiers de paramètres n’ont pas pu être vérifiés et ont été laissés de côté : le serveur écrit ses valeurs par défaut",
        ],
        P::OutcomeScripts => [
            "{count} script a été laissé de côté : seul le propriétaire peut ajouter des scripts, quand l’exécution de code est activée.",
            "{count} scripts ont été laissés de côté : seul le propriétaire peut ajouter des scripts, quand l’exécution de code est activée.",
        ],
        P::ImageMoreNames => ["{count} autre non listé", "{count} autres non listés"],
    }
}
