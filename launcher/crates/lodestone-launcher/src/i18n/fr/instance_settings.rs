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
        M::MemoryChosenAutomatically => {
            "Choisie pour cet ordinateur et les mods de l’instance. Elle suit les mods tant que vous ne la modifiez pas."
        }
        M::WindowDefaultSize => "Taille par défaut du jeu",
        M::WindowFullscreen => "{size}, plein écran",
        M::WindowSizeInvalid => {
            "La taille de la fenêtre demande une largeur et une hauteur en pixels."
        }
        M::WindowSize => "Taille de la fenêtre",
        M::WindowSizeGameDefault => "Par défaut du jeu",
        M::WindowSizeThisScreen => "Cet écran",
        M::WindowSizeCustom => "Personnalisée",
        M::WindowWidth => "Largeur de la fenêtre",
        M::WindowHeight => "Hauteur de la fenêtre",
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
        M::NameMissing => "Donnez un nom à l’instance.",
        M::Icon => "Icône",
        M::PlayBackground => "Fond de l’onglet Jouer",
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
        M::BackgroundDefault => "Par défaut",
        M::BackgroundImage => "Image",
        M::BackgroundSlideshow => "Diaporama",
        M::SlideshowThisInstance => "Cette instance",
        M::SlideshowAllInstances => "Toutes les instances",
        M::SlideshowOwnImages => "Mes images",
        M::AddImages => "Ajouter des images…",
        M::AddToSlideshow => "Ajouter au diaporama",
        M::SlideshowNoScreenshots => {
            "Pas encore de captures d’écran. L’illustration par défaut s’affiche en attendant."
        }
        M::SlideshowNoImages => {
            "Pas encore d’images. Ajoutez-en, ou choisissez des captures ci-dessous."
        }
        M::BlocksAndItems => "Blocs et objets",
        M::Emoji => "Emoji",
        M::UploadImage => "Importer une image…",

        M::AutoGroupServers => "Serveurs",
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
        M::GameVersionChangedMeanwhile => {
            "La version de Minecraft a changé entre-temps. Choisissez à nouveau le chargeur, puis enregistrez."
        }

        M::Vanilla => "Vanilla",
        M::ModLoader => "Chargeur de mods",
        M::AboutVanilla => "Minecraft tel que Mojang le publie.",
        M::AboutFabric => "Léger et vite à jour. La plupart des mods de performance.",
        M::AboutQuilt => "Compatible avec Fabric, avec ses propres extras.",
        M::AboutForge => "Le classique. Gros mods de contenu et anciens modpacks.",
        M::AboutNeoForge => "Le successeur de Forge, pour les versions récentes.",
        M::PrereleaseUpdates => "Mises à jour bêta et alpha",
        M::PrereleaseUpdatesHint => "Désactivé, un mod déjà en bêta reçoit encore des bêtas.",
        M::LoaderVersion => "Version du chargeur",
        M::ChooseLoaderVersion => "Choisissez une version de {loader}.",
        M::ChooseVersion => "Choisir une version",
        M::LoadingLoaderVersions => "Chargement des versions de {loader}",
        M::CouldntLoadVersions => "Impossible de charger les versions\u{a0}: {error}",
        M::NoVersionsFor => "Aucune version pour Minecraft {game}.",
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
        M::DownloadingNewVersion => "Téléchargement de la nouvelle version…",
        M::CheckingModrinth => "Vérification sur Modrinth…",
        M::UpdateTo => "Mettre à jour vers {version}…",
        M::UpToDate => "À jour",
        M::CheckForUpdates => "Rechercher des mises à jour",
        M::ModrinthUnreachable => "Modrinth est injoignable.",
        M::StopGameFirst => "Arrêtez d’abord le jeu, ou attendez la fin de la mise à jour.",
        M::CouldntPrepareUpdate => "Impossible de préparer la mise à jour\u{a0}: {error}",

        M::LinkedServerHint => "Sur {core}. Jouer le rejoint et synchronise ses mods.",
        M::LinkedServerCoreRemoved => {
            "Sur {core}, qui a été retiré de la page Lodestone Host. Ajoutez de nouveau cet hôte sur cette page pour rejoindre le serveur."
        }
        M::CoreRemoved => "Hôte retiré",
        M::Unlink => "Délier",
        M::Unlinked => "Lien supprimé",
        M::UnlinkNote => "Jouer ne rejoindra plus le serveur. Les mods restent.",

        M::ChangeSetting => "Modifier",
        M::JavaMemoryLead => "Les valeurs par défaut viennent des paramètres du launcher.",
        M::Memory => "Mémoire",
        M::Checking => "Vérification…",
        M::JavaFound => "Java {major} (version {version})",
        M::JavaTooOld => {
            "Java {major} (version {version}). Minecraft {game} demande Java {needed} ou plus récent."
        }
        M::MojangJavaVersion => "Java {major} de Mojang",
        M::MojangJava => "Java de Mojang",
        M::AzulZulu8 => "Azul Zulu\u{a0}8, fait pour Apple Silicon",
        M::MojangIntelJavaVersion => "Java {major} Intel de Mojang, grâce à Rosetta\u{a0}2",
        M::JavaTheVersionNeeds => "Le Java dont cette version a besoin",
        M::UseThisJava => "Utiliser ce Java",
        M::Check => "Vérifier",
        M::JvmArguments => "Arguments JVM",
        M::GameArguments => "Arguments du jeu",
        M::NoArguments => "Aucun",
        M::NoVariables => "Aucune",
        M::Window => "Fenêtre",
        M::Fullscreen => "Plein écran",
        M::EnvironmentVariables => "Variables d’environnement",

        M::InstanceSnapshots => "Instantanés",
        M::TakeSnapshotEllipsis => "Prendre un instantané…",
        M::TakeSnapshotTitle => "Prendre un instantané",
        M::TakeSnapshot => "Prendre l’instantané",
        M::SnapshotName => "Nom",
        M::WhatToInclude => "Contenu à inclure",
        M::WhatToRestore => "Contenu à restaurer",
        M::PartMods => "Mods",
        M::PartResourcePacks => "Packs de ressources",
        M::PartShaderPacks => "Packs de shaders",
        M::PartConfigs => "Configurations et options",
        M::PartWorlds => "Mondes",
        M::PartScreenshots => "Captures d’écran",
        M::PartSettings => "Paramètres de l’instance",
        M::PartOther => "Autres fichiers",
        M::NoSnapshots => "Aucun instantané pour l’instant.",
        M::SnapshotSizes => "{total}, dont {new} nouveaux",
        M::BeforeGoingBack => "Avant le retour à «\u{a0}{name}\u{a0}»",
        M::RevertEllipsis => "Restaurer…",
        M::RevertTitle => "Revenir à {name}\u{a0}?",
        M::RevertBody => {
            "Les parties choisies sont remplacées par celles de l’instantané. Un instantané en est pris avant, pour pouvoir revenir en arrière."
        }
        M::Revert => "Restaurer",
        M::ShareEllipsis => "Partager…",
        M::ShareSnapshotTitle => "Partager {name}",
        M::PreparingFile => "Préparation du fichier…",
        M::DragToShare => "Faites glisser le fichier dans une autre app, ou\u{a0}:",
        M::SaveAsEllipsis => "Enregistrer sous…",
        M::ShowInFolder => "Afficher dans le dossier",
        M::DeleteSnapshotTitle => "Supprimer {name}\u{a0}?",
        M::DeleteSnapshotBody => {
            "Les fichiers qu’aucun autre instantané n’utilise sont supprimés. Cette action est définitive."
        }
        M::TakingSnapshot => "Prise de l’instantané…",
        M::RevertingSnapshot => "Restauration de l’instantané…",
        M::ImportingSnapshot => "Création de l’instance…",
        M::ReadingSnapshot => "Lecture de l’instantané…",
        M::ImportSnapshotTitle => "Nouvelle instance depuis un instantané",
        M::SnapshotFrom => "{name} · Minecraft {version}",
        M::SnapshotModsWarning => {
            "Il contient {count} mods\u{a0}: des programmes qui s’exécutent au lancement du jeu. N’utilisez que des instantanés de personnes de confiance."
        }
        M::SnapshotLaunchSettingsWarning => {
            "Ses paramètres d’instance changent le lancement du jeu\u{a0}: Java, ses arguments, les variables d’environnement ou la mémoire. Ils peuvent aussi exécuter des programmes, et ne sont donc utilisés que si Paramètres de l’instance est choisi. Ne le choisissez que pour des instantanés de personnes de confiance."
        }
        M::CreateInstanceButton => "Créer l’instance",
        M::SnapshotGameRunning => "Arrêtez d’abord le jeu.",
        M::ChooseSomething => "Choisissez au moins une partie.",
        M::SnapshotTaken => "Instantané pris.",
        M::SnapshotReverted => "Instantané restauré.",
        M::SnapshotsHint => "Des copies de cette instance, pour revenir en arrière ou partager.",
        M::VerifyGameFiles => "Vérifier les fichiers du jeu",
        M::VerifyHint => "Retélécharge les fichiers manquants ou endommagés.",
        M::Verify => "Vérifier",
        M::VerifyStarted => {
            "Vérification de chaque fichier. La progression s’affiche sur le bouton Jouer."
        }
        M::FindCrashingMod => "Trouver un mod qui fait planter le jeu",
        M::FindCrashingModHint => {
            "Lance le jeu plusieurs fois avec la moitié des mods désactivés, jusqu’à ce qu’il n’en reste qu’un."
        }
        M::FindCrashingModButton => "Trouver…",
        M::FindCrashingModKeywords => "plantage crash conflit cassé incompatible dépannage",
        M::Export => "Exporter",
        M::ExportHint => "En fichier .mrpack.",
        M::ExportButton => "Exporter…",
        M::Exporting => "Exportation…",
        M::Duplicate => "Dupliquer",
        M::Duplicating => "Duplication…",
        M::HostOnCore => "Héberger sur Lodestone",
        M::HostHint => "Crée un serveur avec les mods de cette instance.",
        M::HostButton => "Héberger…",
        M::ShareCodeRow => "Code de partage",
        M::ShareButton => "Partager…",
        M::ShareTitle => "Partager {name}",
        M::ShareIntro => {
            "Un autre Lodestone Launcher peut recréer cette instance avec ce code\u{a0}: la même version de Minecraft, le même chargeur et le même contenu Modrinth."
        }
        M::ShareReading => "Lecture du contenu…",
        M::ShareNotSharedBody => {
            "Seul le contenu de Modrinth peut figurer dans un code de partage. Envoyez ces fichiers à part s’ils sont nécessaires."
        }
        M::ShareFromCurseForge => "Depuis CurseForge",
        M::ShareAddedByHand => "Ajouté à la main",
        M::ShareNotOnModrinth => "Absent de Modrinth",
        M::ShareChangedSincePack => "Modifié depuis le modpack",
        M::ShareUnchecked => "Non vérifié auprès de Modrinth",
        M::ShareBasedOnPack => "Basé sur le modpack {pack}",
        M::ShareIntroPack => {
            "Un autre Lodestone Launcher peut recréer cette instance avec ce code\u{a0}: le modpack, puis vos modifications."
        }
        M::ShareNotSharedBodyPack => {
            "Le modpack apporte ses propres fichiers, mais ceux-ci n’arriveront pas tels quels. Envoyez-les à part s’ils sont nécessaires."
        }
        M::ShareNoChanges => "Aucune modification du modpack",
        M::ShareChanges => "Vos modifications\u{a0}: {changes}",
        M::CodePackPreview => "{pack} depuis Modrinth",
        M::ShareMore => "Et {count} autres",
        M::CopyCode => "Copier le code",
        M::CodeCopied => "Copié",
        M::ShareCodeMode => "Code de partage",
        M::PasteShareCode => "Collez un code de partage",
        M::CodeNotACode => "Ce n’est pas un code de partage Lodestone.",
        M::CodeNewer => "Ce code vient d’une version plus récente de Lodestone Launcher.",
        M::CodeTooLarge => "Le code est trop long.",
        M::CodeDamaged => "Le code est endommagé. Copiez-le à nouveau.",
        M::DeleteInstance => "Supprimer l’instance",
        M::DeleteHint => "Y compris ses mondes et ses mods.",
        M::DeleteButton => "Supprimer…",

        M::SharedLead => "Gardez vos réglages synchronisés entre vos instances.",
        M::UseSharedSettings => "Utiliser les paramètres partagés",
        M::SharingOff => "Cette instance garde ses propres options.",
        M::SharingFrom => "Reçoit les changements faits en jeu dans {name}.",
        M::SharingMain => "Les changements faits en jeu ici mettent à jour toutes les instances.",
        M::SharingNoMain => {
            "Aucune instance principale\u{a0}: les valeurs partagées ne changent que dans les Paramètres."
        }
        M::MainInstance => "Instance principale",
        M::MainInstanceHint => {
            "Les réglages modifiés en jeu dans votre instance principale sont copiés dans chaque instance qui les partage, à son prochain démarrage. Une seule instance est principale."
        }
        M::IsMainInstance => "C’est votre instance principale",
        M::MakeMain => "Rendre principale",
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
        M::SharedNotApplied => {
            "Les paramètres de jeu partagés n’ont pas été appliqués\u{a0}: {error}"
        }
        M::SharedNotReadBack => {
            "Impossible de relire les paramètres de jeu partagés\u{a0}: {error}"
        }
        M::CategoryControls => "Commandes",
        M::CategoryVideo => "Vidéo",
        M::CategorySound => "Son",
        M::CategoryAccessibility => "Accessibilité et chat",
        M::CategoryLanguage => "Langue",
        M::CategoryControlsDescription => "Souris, touches",
        M::CategoryVideoDescription => {
            "Champ de vision, distance d’affichage, taille de l’interface, graphismes"
        }
        M::CategorySoundDescription => "Volume, sous-titres",
        M::CategoryAccessibilityDescription => "Narrateur, chat, fond du texte",
        M::CategoryLanguageDescription => "Langue et police du jeu",

        M::NewInstance => "Nouvelle instance",
        M::MinecraftVersion => "Version de Minecraft",
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
        M::PickModpack => "Choisissez un modpack et une version.",
        M::ChooseGameVersion => "Choisissez une version de Minecraft.",
        M::CreateInstance => "Créer l’instance",
        M::InstallModpack => "Installer le modpack",
        M::ImportMrpack => "Importer un .mrpack…",
        M::Import => "Importer",
        M::MemoryKeywords => "RAM mémoire vive Xmx Xms",
        M::JavaKeywords => "JDK JRE environnement d’exécution chemin",
        M::JvmArgumentsKeywords => "Java options paramètres",
        M::WindowKeywords => "résolution largeur hauteur plein écran taille",
        M::EnvironmentKeywords => "env PATH",
        M::PlayBackgroundKeywords => "fond d’écran image photo diaporama",
        M::DeleteKeywords => "supprimer retirer désinstaller",
        M::LoaderKeywords => "chargeur de mods Fabric Forge NeoForge Quilt",

        M::AddModpackTitle => "Ajouter un modpack à {name}",
        M::AddModpackFilter => "Modpacks pour Minecraft {version} avec {loader}. {note}",
        M::AddModpackServerNote => {
            "Les mods du pack s’ajoutent à ceux de {server}, qui restent tels que le serveur les a\u{a0}: vous pouvez toujours le rejoindre."
        }
        M::AddModpackNote => {
            "Les fichiers du pack s’ajoutent à ceux de l’instance. Rien de ce qu’elle a n’est remplacé."
        }
        M::AddModpackButton => "Ajouter le modpack",
        M::CheckingModpack => "Vérification du modpack",
        M::AddModpackSummary => "{pack} {version}\u{a0}: {size} à télécharger.",
        M::AddModpackNothing => "L’instance a déjà tous les fichiers de ce pack.",
        M::AddGroupFiles => "À ajouter ({count})",
        M::AddGroupConfigs => "Configurations à ajouter ({count})",
        M::AddGroupServer => "Gardés tels que {server} les a ({count})",
        M::AddGroupPack => "Gardés tels que le modpack de l’instance les a ({count})",
        M::AddGroupThere => "Déjà dans l’instance ({count})",
        M::AddedModpacks => "Modpacks ajoutés",
        M::AddModpackRow => "Ajouter un modpack",
        M::AddModpackHint => {
            "Ajoutez les mods et configurations d’un modpack à cette instance, à côté des siens."
        }
        M::AddModpackServerHint => {
            "Ajoutez les mods d’un modpack, comme des optimisations côté client, à côté de ceux de {server}. Les mods du serveur restent tels quels."
        }
        M::AddModpackEllipsis => "Ajouter un modpack…",
        M::RemoveAddedModpackNote => {
            "Supprime les fichiers ajoutés par le pack. Les fichiers que vous avez modifiés restent."
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ShareNotShared => [
            "{count} fichier n’est pas partagé",
            "{count} fichiers ne sont pas partagés",
        ],
        P::ShareAddedCount => ["{count} ajouté", "{count} ajoutés"],
        P::ShareRemovedCount => ["{count} retiré", "{count} retirés"],
        P::ShareChangedCount => ["{count} modifié", "{count} modifiés"],
        P::ShareToggledCount => [
            "{count} activé ou désactivé",
            "{count} activés ou désactivés",
        ],
        P::CodePackChanges => ["+ {count} modification", "+ {count} modifications"],
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
        P::SharedWritten => [
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeur écrite dans options.txt",
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeurs écrites dans options.txt",
        ],
        P::SharedWrittenNewFile => [
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeur écrite dans options.txt (nouveau fichier)",
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeurs écrites dans options.txt (nouveau fichier)",
        ],
        P::SharedEmitted => [
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeur de cette instance s’applique désormais à toutes les instances",
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}valeurs de cette instance s’appliquent désormais à toutes les instances",
        ],
        P::SharedChangedNote => [
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}paramètre changé en jeu sera remplacé par la valeur partagée au prochain lancement (voir Paramètres)",
            "Paramètres de jeu partagés\u{a0}: {count}\u{a0}paramètres changés en jeu seront remplacés par les valeurs partagées au prochain lancement (voir Paramètres)",
        ],

        P::AndMoreFiles => ["et {count} autre", "et {count} autres"],
        P::AddedModpackFiles => ["{count} fichier ajouté", "{count} fichiers ajoutés"],
    }
}
