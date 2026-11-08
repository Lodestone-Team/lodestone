use crate::i18n::{InstanceSettingsMsg as M, InstanceSettingsPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::MemoryFormat => {
            "Indiquez la mémoire en Mo (par exemple 4096) ou en Go (par exemple 4G), au moins 256\u{a0}Mo."
        }
        M::MemoryMinAboveMax => "Le minimum dépasse le maximum.",
        M::MemoryTooHigh => {
            "C’est plus des trois quarts de la mémoire de {computer}, ce qui laisse trop peu pour {os} et vos autres apps."
        }
        M::TotalMemory => "{computer} a {size} de mémoire.",
        M::MemoryRange => "{min} – {max}",
        M::MemoryUnit => "Mo",
        M::WindowDefaultSize => "Taille par défaut du jeu",
        M::WindowFullscreen => "{size}, plein écran",
        M::WindowSizeInvalid => {
            "La taille de la fenêtre demande une largeur et une hauteur en pixels."
        }
        M::EnvInvalid => "Variables d’environnement\u{a0}: {error}",
        M::EnvPlaceholder => "NOM=valeur",

        M::General => "Général",
        M::GameAndLoader => "Jeu et chargeur",
        M::JavaAndMemory => "Java et mémoire",
        M::LaunchOptions => "Options de lancement",
        M::SharedGameSettings => "Paramètres de jeu partagés",
        M::Maintenance => "Maintenance",
        M::Modpack => "Modpack",
        M::LinkedServer => "Serveur lié",

        M::Name => "Nom",
        M::Group => "Groupe",
        M::NoGroup => "Aucun groupe",
        M::GroupHintAuto => {
            "Les instances d’un même groupe sont listées ensemble. Sans groupe, celle-ci est listée sous {group}."
        }
        M::GroupHint => "Les instances d’un même groupe sont listées ensemble.",
        M::NameMissing => "Donnez un nom à l’instance.",
        M::Icon => "Icône",
        M::IconHint => "Affichée dans la barre latérale et ailleurs dans le launcher.",
        M::PlayBackground => "Fond de l’onglet Jouer",
        M::PlayBackgroundHint => {
            "Affiché derrière le nom dans l’onglet Jouer. Le launcher l’assombrit sous le texte autant que l’image le demande."
        }
        M::UseAsIcon => "Utiliser comme icône",
        M::UseAsBackground => "Utiliser comme fond",
        M::SavedSeePlayTab => "Enregistré. Voyez le résultat dans l’onglet Jouer.",
        M::YourScreenshots => "Vos captures d’écran",
        M::FindingScreenshots => "Recherche de vos captures d’écran",
        M::NoScreenshots => {
            "Pas encore de capture d’écran. Appuyez sur F2 en jeu pour en prendre une, ou choisissez un fichier."
        }
        M::ChooseFile => "Choisir un fichier…",
        M::UseDefault => "Utiliser celui par défaut",
        M::BlocksAndItems => "Blocs et objets",
        M::Emoji => "Emoji",
        M::UploadImage => "Importer une image…",

        M::AutoGroupFabric => "Fabric (moddé)",
        M::AutoGroupForge => "Forge (moddé)",
        M::AutoGroupNeoForge => "NeoForge (moddé)",
        M::AutoGroupQuilt => "Quilt (moddé)",
        M::AutoGroupVanilla => "Vanilla",
        M::AutoGroupImported => "Importées",

        M::GameVersionHint => "Choisie à la création de l’instance.",
        M::LoaderInstallsNextStart => "{loader} s’installera au prochain démarrage du jeu.",
        M::LoaderInstallsNextPlay => {
            "{loader} s’installera la prochaine fois que vous cliquerez sur Jouer."
        }
        M::SavedNextStart => "Enregistré. Le changement s’applique au prochain démarrage du jeu.",
        M::CouldntTurnModsOff => "Impossible de désactiver les mods\u{a0}: {error}",

        M::Vanilla => "Vanilla",
        M::ModLoader => "Chargeur de mods",
        M::ModLoaderHint => {
            "Un changement s’installe la prochaine fois que vous cliquez sur Jouer."
        }
        M::LoaderVersion => "Version du chargeur",
        M::ChooseLoaderVersion => "Choisissez une version de {loader}.",
        M::ChooseVersion => "Choisir une version",
        M::LoadingLoaderVersions => "Chargement des versions de {loader}",
        M::CouldntLoadVersions => "Impossible de charger les versions\u{a0}: {error}",
        M::CouldntLoadLoaderVersions => {
            "Impossible de charger les versions de {loader}\u{a0}: {error}"
        }
        M::NoVersionsFor => "Aucune version pour Minecraft {game}.",
        M::LoaderHasNoVersions => "{loader} n’a aucune version pour Minecraft {game}.",
        M::PlainMinecraft => "Minecraft tel quel, sans mods.",
        M::ChooseGameVersionFirst => "Choisissez d’abord une version de Minecraft.",
        M::Recommended => "Recommandée",
        M::Beta => "Bêta",

        M::FromModrinth => "Depuis Modrinth",
        M::FromServerMods => {
            "Créé à partir des mods de {server}\u{a0}: Jouer les garde à jour avec le serveur."
        }
        M::FromMrpack => {
            "Depuis un fichier .mrpack absent de Modrinth\u{a0}: il ne peut pas être mis à jour ici."
        }
        M::ViewOnModrinth => "Voir sur Modrinth",
        M::Updates => "Mises à jour",
        M::UpdatesHint => {
            "Une mise à jour remplace les fichiers du modpack et garde vos mods, configurations et mondes. Vous voyez d’abord ce qui change."
        }
        M::DownloadingNewVersion => "Téléchargement de la nouvelle version…",
        M::CheckingModrinth => "Vérification sur Modrinth…",
        M::UpdateTo => "Mettre à jour vers {version}…",
        M::UpToDate => "À jour",
        M::CheckForUpdates => "Rechercher des mises à jour",
        M::ModrinthUnreachable => "Modrinth est injoignable.",
        M::StopGameFirst => "Arrêtez d’abord le jeu, ou attendez la fin de la mise à jour.",
        M::CouldntPrepareUpdate => "Impossible de préparer la mise à jour\u{a0}: {error}",

        M::LinkedServerHint => "Sur {core}. Jouer le rejoint et garde les mods à jour.",
        M::LinkedServerCoreRemoved => {
            "Sur {core}, qui a été retiré de la page Serveurs. Ajoutez de nouveau ce core sur cette page pour rejoindre le serveur."
        }
        M::CoreRemoved => "Core retiré",
        M::Unlink => "Délier",
        M::Unlinked => "Lien supprimé",
        M::UnlinkNote => {
            "Délier garde l’instance et ses mods. Jouer ne rejoindra plus le serveur et ne mettra plus ses mods à jour."
        }

        M::ChangeSetting => "Modifier",
        M::JavaMemoryLead => {
            "Les valeurs marquées Par défaut viennent des Paramètres du launcher. Modifiez-en une pour la définir pour cette instance seulement."
        }
        M::Memory => "Mémoire",
        M::Checking => "Vérification…",
        M::JavaFound => "Java {major} (version {version})",
        M::JavaTooOld => {
            "Java {major} (version {version}). Minecraft {game} demande Java {needed} ou plus récent."
        }
        M::MojangJavaVersion => "Java {major} de Mojang",
        M::MojangJava => "Java de Mojang",
        M::JavaOverrideHint => {
            "Un exécutable java à utiliser à la place de celui fourni par Mojang."
        }
        M::JavaHint => "Mojang fournit le bon Java pour chaque version de Minecraft.",
        M::UseThisJava => "Utiliser ce Java",
        M::Check => "Vérifier",
        M::JvmArguments => "Arguments JVM",
        M::GameArguments => "Arguments du jeu",
        M::NoArguments => "Aucun",
        M::NoVariables => "Aucune",
        M::ArgumentsHint => {
            "Séparés par des espaces\u{202F}; mettez entre guillemets les valeurs avec des espaces."
        }
        M::Window => "Fenêtre",
        M::Fullscreen => "Plein écran",
        M::WindowHint => "Laissez la taille vide pour garder celle du jeu par défaut.",
        M::EnvironmentVariables => "Variables d’environnement",
        M::EnvHint => "NOM=valeur, séparées par des espaces.",

        M::VerifyGameFiles => "Vérifier les fichiers du jeu",
        M::VerifyHint => {
            "Vérifie chaque fichier et télécharge ceux qui manquent ou sont endommagés."
        }
        M::Verify => "Vérifier",
        M::VerifyStarted => {
            "Vérification de chaque fichier. La progression s’affiche sur le bouton Jouer."
        }
        M::Export => "Exporter",
        M::ExportHint => "Un fichier .mrpack que d’autres launchers peuvent importer.",
        M::ExportButton => "Exporter…",
        M::Exporting => "Exportation…",
        M::Duplicate => "Dupliquer",
        M::DuplicateHint => "Une copie avec son propre dossier, ses mods et ses mondes.",
        M::HostOnCore => "Héberger sur un Lodestone Core",
        M::HostHint => "Crée un serveur sur l’un de vos cores avec les mods de cette instance.",
        M::HostButton => "Héberger…",
        M::DeleteInstance => "Supprimer l’instance",
        M::DeleteHint => {
            "Son dossier, avec ses mondes et ses mods. Une confirmation vous est demandée."
        }
        M::DeleteButton => "Supprimer…",

        M::SharedLead => {
            "Souris, touches, vidéo, son et plus encore depuis options.txt, identiques dans chaque instance."
        }
        M::UseSharedSettings => "Utiliser les paramètres partagés",
        M::SharingOff => {
            "Désactivé\u{a0}: cette instance garde son propre options.txt, et rien n’en est relu."
        }
        M::SharingPreferred => {
            "C’est l’instance préférée\u{a0}: les paramètres que vous changez en jeu ici s’appliquent à chaque instance."
        }
        M::SharingFrom => {
            "Les paramètres changés en jeu dans {name} arrivent dans cette instance à son prochain lancement."
        }
        M::SharingNoPreferred => {
            "Aucune instance n’est préférée\u{a0}: les valeurs partagées ne changent que dans les Paramètres du launcher."
        }
        M::Preferred => "Préférée",
        M::MakePreferred => "Rendre préférée",
        M::MadePreferred => {
            "Les changements faits en jeu dans cette instance s’appliquent désormais à chaque instance."
        }
        M::StopGameToChange => {
            "Arrêtez le jeu pour modifier ceci\u{a0}: il réécrit options.txt en quittant."
        }
        M::NotShared => "Non partagé (Paramètres du launcher)",
        M::ShareAll => "Tout partager",
        M::OverrideAll => "Tout remplacer",
        M::Show => "Afficher",
        M::Hide => "Masquer",
        M::NotSharedYet => "Pas encore partagé",
        M::NotInOptions => "Absent d’options.txt",
        M::ThisInstance => "Cette instance",
        M::UseShared => "Utiliser la valeur partagée ({value})",
        M::Shared => "Partagé",
        M::Override => "Remplacer",
        M::Value => "Valeur",
        M::KeepForInstance => "Les garder pour cette instance",
        M::Dismiss => "Ignorer",
        M::CategoryControls => "Commandes",
        M::CategoryVideo => "Vidéo",
        M::CategorySound => "Son",
        M::CategoryAccessibility => "Accessibilité et chat",
        M::CategoryLanguage => "Langue",
        M::CategoryControlsDescription => {
            "Sensibilité et inversion de la souris, entrée brute, touches, accroupissement et sprint en bascule"
        }
        M::CategoryVideoDescription => {
            "Champ de vision, distances d’affichage et de simulation, taille de l’interface, luminosité, plein écran, VSync, FPS max, particules, graphismes"
        }
        M::CategorySoundDescription => {
            "Tous les curseurs de volume, sous-titres, audio directionnel"
        }
        M::CategoryAccessibilityDescription => {
            "Narrateur, fond du texte, taille et opacité du chat, effets"
        }
        M::CategoryLanguageDescription => "Langue et police du jeu",

        M::NewInstance => "Nouvelle instance",
        M::MinecraftVersion => "Version de Minecraft",
        M::NameLabel => "NOM",
        M::MinecraftVersionLabel => "VERSION DE MINECRAFT",
        M::ModLoaderLabel => "CHARGEUR DE MODS",
        M::MyInstance => "Mon instance",
        M::SearchVersions => "Rechercher une version",
        M::Releases => "Versions stables",
        M::Snapshots => "Snapshots",
        M::OldVersions => "Anciennes versions",
        M::KindRelease => "Stable",
        M::KindSnapshot => "Snapshot",
        M::KindBeta => "Bêta",
        M::KindAlpha => "Alpha",
        M::KindOther => "Autre",
        M::LoadingVersionList => "Chargement de la liste des versions de Mojang",
        M::NoVersionsMatch => "Aucune version ne correspond.",
        M::GameWithLoader => "Minecraft {version} avec {loader} {loader_version}",
        M::NoVersionSelected => "Aucune version choisie",
        M::NoModpackPicked => "Aucun modpack choisi",
        M::PickModpack => "Choisissez un modpack et une version.",
        M::ChooseGameVersion => "Choisissez une version de Minecraft.",
        M::CreateInstance => "Créer l’instance",
        M::InstallModpack => "Installer le modpack",
        M::ImportMrpack => "Importer un .mrpack…",
        M::Import => "Importer",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::LoaderNextStartModsOff => [
            "{loader} s’installera au prochain démarrage du jeu. {count}\u{a0}mod a été désactivé.",
            "{loader} s’installera au prochain démarrage du jeu. {count}\u{a0}mods ont été désactivés.",
        ],
        P::LoaderNextPlayModsOff => [
            "{loader} s’installera la prochaine fois que vous cliquerez sur Jouer. {count}\u{a0}mod a été désactivé.",
            "{loader} s’installera la prochaine fois que vous cliquerez sur Jouer. {count}\u{a0}mods ont été désactivés.",
        ],
        P::SharedCount => ["{count}\u{a0}partagé", "{count}\u{a0}partagés"],
        P::OverriddenCount => ["{count}\u{a0}remplacé", "{count}\u{a0}remplacés"],
        P::AndMore => [
            "{names} et {count}\u{a0}autre",
            "{names} et {count}\u{a0}autres",
        ],
        P::ChangedInGame => [
            "{count}\u{a0}paramètre changé en jeu ({names}) sera remplacé par la valeur partagée au prochain lancement.",
            "{count}\u{a0}paramètres changés en jeu ({names}) seront remplacés par les valeurs partagées au prochain lancement.",
        ],
    }
}
