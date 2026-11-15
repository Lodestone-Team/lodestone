use crate::i18n::{SharingMsg as M, SharingPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ImageMadeByHost => "Créé avec Lodestone Host {version}",
        M::ImageMadeByLauncher => "Créé avec Lodestone Launcher {version}",
        M::ImageMadeByOther => "Créé par un autre programme",
        M::ImageCarried => {
            "Ceux-ci sont dans le fichier lui-même, et non téléchargés depuis Modrinth : {names}"
        }
        M::ImageClientOnly => "Mods que seuls les joueurs utilisent, laissés de côté : {names}",
        M::ImageNotDownloadable => {
            "Les serveurs Lodestone ne peuvent pas encore télécharger depuis CurseForge, donc ces fichiers ne seront pas installés : {names}"
        }
        M::ImageTakenOut => "Retiré par l’expéditeur : {names}",
        M::ImageBlanked => "Vidé par l’expéditeur : {names}",
        M::ImageSecretsNotTaken => {
            "Non repris, car ils contiennent une clé du serveur, un mot de passe ou un jeton : {names}"
        }
        M::ImageWithheld => "Absents du fichier : {names}",
        M::ImageNotTaken => "Non repris du fichier : {what}",
        M::NotTakenSoftware => "le logiciel du serveur (Lodestone installe le sien)",
        M::NotTakenEula => "le CLUF",
        M::NotTakenClientFiles => "les fichiers propres à un joueur",
        M::NotTakenOperators => "les opérateurs",
        M::NotTakenScripts => "les scripts",
        M::ImageOfflineMode => {
            "Ses paramètres permettent de rejoindre sans compte Minecraft (online-mode est désactivé)."
        }
        M::ImageServerSoftware => "Logiciel serveur",
        M::ImageServerSoftwareHint => {
            "Paper exécute des plugins. Un serveur vanilla exécute Minecraft tel quel."
        }
        M::ImageWorld => "Monde",
        M::ImageWorldHint => "Le monde qui devient celui du serveur.",
        M::ImageNoWorld => "Aucun monde (un nouveau)",
        M::ImageDownloads => "Télécharge {size} depuis Modrinth",
        M::ImageDownloadsMods => "Télécharge {size} depuis Modrinth : {names}",
        M::ImageDisk => "Nécessite {needed} d’espace disque, dont 1 Go de marge ; {free} libres.",
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
            "Lodestone crée les serveurs {flavour} comme des serveurs Paper, qui font tourner les mêmes plugins."
        }
        M::ImageUnsupported => "Lodestone ne peut pas créer de serveurs {loader}.",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ImageModsWarning => [
            "Il contient {count} mod ou plugin : un programme qui s’exécute sur le serveur. N’utilisez que des fichiers de personnes de confiance.",
            "Il contient {count} mods ou plugins : des programmes qui s’exécutent sur le serveur. N’utilisez que des fichiers de personnes de confiance.",
        ],
        P::OutcomeNotDownloaded => [
            "{count} fichier n’a pas pu être téléchargé et a été laissé de côté : {names}",
            "{count} fichiers n’ont pas pu être téléchargés et ont été laissés de côté : {names}",
        ],
        P::OutcomeNoSource => [
            "{count} fichier ne peut pas être téléchargé par les serveurs Lodestone et a été laissé de côté : {names}",
            "{count} fichiers ne peuvent pas être téléchargés par les serveurs Lodestone et ont été laissés de côté : {names}",
        ],
        P::OutcomeClientJars => [
            "{count} mod qui ne fonctionne que sur l’ordinateur des joueurs a été retiré : {names}",
            "{count} mods qui ne fonctionnent que sur l’ordinateur des joueurs ont été retirés : {names}",
        ],
        P::OutcomeSecrets => [
            "{count} fichier contenant une clé du serveur, un mot de passe ou un jeton a été laissé de côté ; le serveur écrit ses valeurs par défaut : {names}",
            "{count} fichiers contenant une clé du serveur, un mot de passe ou un jeton ont été laissés de côté ; le serveur écrit ses valeurs par défaut : {names}",
        ],
        P::OutcomeSettingsRemoved => [
            "{count} fichier de paramètres n’a pas pu être vérifié (clés du serveur) et a été laissé de côté ; le serveur écrit ses valeurs par défaut : {names}",
            "{count} fichiers de paramètres n’ont pas pu être vérifiés (clés du serveur) et ont été laissés de côté ; le serveur écrit ses valeurs par défaut : {names}",
        ],
        P::OutcomeScripts => [
            "{count} script a été laissé de côté : seul le propriétaire peut ajouter des scripts, quand l’exécution de code est activée.",
            "{count} scripts ont été laissés de côté : seul le propriétaire peut ajouter des scripts, quand l’exécution de code est activée.",
        ],
    }
}
