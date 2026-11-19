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

        M::GameVersionHint => "Ne change plus une fois l’instance créée.",
        M::GameVersionFollowsModpack => "Suit les mises à jour de son modpack.",
        M::GameVersionFollowsServer => "Suit le pack du serveur lié.",
        M::LoaderInstallsNextStart => "{loader} s’installera au prochain démarrage du jeu.",
        M::LoaderInstallsNextPlay => {
            "{loader} s’installera la prochaine fois que vous cliquerez sur Jouer."
        }
        M::SavedNextStart => "Enregistré. Le changement s’applique au prochain démarrage du jeu.",

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
        M::FromCurseForge => "Depuis CurseForge",
        M::FromServerMods => {
            "Créé à partir des mods de {server}\u{a0}: Jouer les garde à jour avec le serveur."
        }
        M::FromMrpack => {
            "Depuis un fichier .mrpack absent de Modrinth\u{a0}: il ne peut pas être mis à jour ici."
        }
        M::ViewOnModrinth => "Voir sur Modrinth",
        M::ViewOnCurseForge => "Voir sur CurseForge",
        M::DownloadingNewVersion => "Téléchargement de la nouvelle version…",
        M::CheckingModrinth => "Vérification sur Modrinth…",
        M::CheckingCurseForge => "Vérification sur CurseForge…",
        M::UpdateModpack => "Mettre à jour…",
        M::UpToDate => "À jour",
        M::CheckForUpdates => "Rechercher des mises à jour",
        M::ModrinthUnreachable => "Modrinth est injoignable.",
        M::CurseForgeUnreachable => "CurseForge est injoignable.",
        M::StopGameFirst => "Arrêtez d’abord le jeu, ou attendez la fin de la mise à jour.",
        M::CouldntPrepareUpdate => "Impossible de préparer la mise à jour\u{a0}: {error}",

        M::LinkedServerHint => "Jouer le rejoint et synchronise ses mods.",
        M::LinkedServerCoreRemoved => {
            "Son hôte a été retiré de la page Lodestone Host. Ajoutez de nouveau l’hôte sur cette page pour rejoindre le serveur."
        }
        M::CoreRemoved => "Hôte retiré",
        M::Unlink => "Délier",
        M::Unlinked => "Lien supprimé",
        M::UnlinkNote => "Jouer ne rejoindra plus le serveur. Les mods restent.",

        M::ChangeSetting => "Modifier",
        M::JavaMemoryLead => {
            "La mémoire par défaut se règle dans Paramètres › Paramètres par défaut des instances."
        }
        M::Memory => "Mémoire",
        M::Checking => "Vérification…",
        M::JavaTooOldBadge => "Trop ancien",
        M::JavaFitsBadge => "Convient",
        M::JavaFoundKey => "Trouvé",
        M::JavaVersion => "Java {version}",
        M::JavaOrNewer => "Java {major} ou plus récent",
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
        M::NoSnapshots => "Aucun instantané pour l’instant.",
        M::SnapshotSizes => "{total}, dont {new} nouveaux",
        M::SnapshotNothingNew => "{total}, rien de nouveau",
        M::PreparingFile => "Préparation du fichier…",
        M::DragToShare => "Faites glisser le fichier dans une autre app, ou\u{a0}:",
        M::SaveAsEllipsis => "Enregistrer sous…",
        M::ShowInFolder => "Afficher dans le dossier",
        M::DeleteSnapshotTitle => "Supprimer {name}\u{a0}?",
        M::DeleteSnapshotBody => {
            "L’instantané est supprimé. Ses fichiers partent au prochain nettoyage, sauf si un autre instantané les utilise. Cette action est irréversible."
        }
        M::TakingSnapshot => "Prise de l’instantané…",
        M::ImportingSnapshot => "Création de l’instance…",
        M::ReadingSnapshot => "Lecture de l’instantané…",
        M::ImportSnapshotTitle => "Nouvelle instance depuis un instantané",
        M::ImportServerTitle => "Nouvelle instance depuis le monde d’un serveur",
        M::CreateInstanceButton => "Créer l’instance",
        M::SnapshotGameRunning => "Arrêtez d’abord le jeu.",
        M::SnapshotAfterPlay => "Prendre un instantané après chaque partie",
        M::BadgeAutomatic => "Automatique",
        M::BadgeAfterPlaying => "Après une partie",
        M::BadgeImported => "Importé",
        M::BadgePinned => "Épinglé",
        M::RestoreEllipsis => "Restaurer…",
        M::ExportEllipsis => "Exporter…",
        M::PinSnapshot => "Épingler",
        M::UnpinSnapshot => "Désépingler",
        M::UnreadableImagesHint => {
            "L’ancien historique du contenu n’est pas nettoyé tant qu’une image est illisible."
        }
        M::DeleteUnreadable => "Les supprimer",
        M::RestoreTitle => "Restaurer {name}",
        M::RestoreWorldsOnly => "Les mondes seulement",
        M::RestoreWorldsAndMods => "Les mondes et leurs mods",
        M::RestoreEverything => "Tout",
        M::RestoreAsNew => "Comme nouvelle instance",
        M::RestoreAsNewName => "Nom de la nouvelle instance",
        M::RestoreEverythingHint => {
            "Tout le dossier du jeu revient à l’instantané\u{a0}: mondes, mods, configurations et options. Les mondes créés depuis sont supprimés."
        }
        M::WorldGoneNow => "{size} · plus dans l’instance",
        M::ModsDiffer => "Ces mondes ont tourné pour la dernière fois avec d’autres mods.",
        M::FilesDiffer => "Ces mondes ont tourné pour la dernière fois avec d’autres fichiers.",
        M::RestoreModsToo => "Remettre aussi leurs mods",
        M::RestoreModsTooChanges => "En remettant aussi leurs mods\u{a0}:",
        M::RestoreLoaderChange => "Le chargeur revient à la version {version}.",
        M::RestoreSafety => {
            "Un instantané de ce qui est remplacé est pris d’abord et gardé 30 jours."
        }
        M::RestoreOtherGame => {
            "Cet instantané est d’une autre version de Minecraft ou d’un autre chargeur\u{a0}: il ne peut devenir qu’une nouvelle instance."
        }
        M::RestoreButton => "Restaurer",
        M::RestoringSnapshot => "Restauration de l’instantané…",
        M::DeletingSnapshot => "Suppression de l’instantané…",
        M::ChooseWorld => "Choisissez au moins un monde.",
        M::ExportTitle => "Exporter {name}",
        M::ExportNote => {
            "Les mods et packs de Modrinth et CurseForge sont listés, pas copiés\u{a0}: ils sont téléchargés de nouveau à l’import du fichier, quand leurs auteurs le permettent."
        }
        M::ExportLeftOut => {
            "Laissés de côté, car ils peuvent contenir des mots de passe et des connexions\u{a0}: {files}."
        }
        M::ImportFrom => "{name} · Minecraft {version} · {loader}",
        M::ImportWorlds => "Mondes\u{a0}: {worlds}",
        M::ImportDownloads => "{size} à télécharger depuis Modrinth et CurseForge.",
        M::ImportNotDownloadable => {
            "Leurs auteurs n’autorisent pas le téléchargement, ils sont donc laissés de côté\u{a0}: {files}."
        }
        M::ImportServerOnlyMods => {
            "Mods que seul le serveur utilise, laissés de côté\u{a0}: {names}"
        }
        M::ImportServerPack => {
            "Les fichiers que seuls les joueurs du serveur utilisent viennent de son modpack sur Modrinth et peuvent contenir des mods\u{a0}: des programmes qui s’exécutent au démarrage du jeu. Ne téléchargez que des mondes de serveurs de confiance."
        }
        M::ImportServerPackCurseforge => {
            "Le modpack du serveur vient de CurseForge, qui ne peut pas encore être téléchargé depuis un serveur\u{a0}: les fichiers que seuls ses joueurs utilisent ne viendront pas."
        }
        M::ImportHostLeftOut => "L’hôte a laissé ceux-ci hors du fichier\u{a0}: {files}",
        M::ImportHostBlanked => "L’hôte a vidé ces secrets\u{a0}: {names}",
        M::ImportSpace => "Nécessite {size} d’espace disque, dont 1\u{a0}Go de marge.",
        M::ImportNeedsModrinth => {
            "Modrinth est injoignable\u{a0}: les mods du fichier ne peuvent pas être téléchargés."
        }
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
        M::ExportHintBoth => "En fichier .mrpack de Modrinth, ou en .zip de CurseForge.",
        M::ExportButton => "Exporter…",
        M::ExportCurseForgeButton => "CurseForge .zip…",
        M::ExportModrinthButton => "Modrinth .mrpack…",
        M::Exporting => "Exportation…",
        M::Duplicate => "Dupliquer",
        M::Duplicating => "Duplication…",
        M::ChangeVersionOrLoader => "Une autre version ou un autre chargeur",
        M::ChangeVersionOrLoaderHint => "Crée une copie. Cette instance reste telle quelle.",
        M::CopyAs => "Copier avec une autre version ou un autre chargeur",
        M::CopyAsButton => "Copier…",
        M::CopyAsKeywords => {
            "changer passer mettre à jour monter descendre version chargeur Fabric Forge NeoForge Quilt"
        }
        M::CopyAsTitle => "Copier avec une autre version ou un autre chargeur",
        M::CopyFrom => {
            "Une nouvelle instance à partir de {name} ({version}), qui reste telle quelle."
        }
        M::CopyWhat => "Ce qui est copié",
        M::CopyWorlds => "Mondes",
        M::CopyNoWorlds => "Cette instance n’a aucun monde.",
        M::CopyWorldsOlder => {
            "Les mondes ne peuvent pas passer à une version plus ancienne de Minecraft\u{a0}: elle ne peut pas les ouvrir."
        }
        M::CopyWorldsUnknown => {
            "Les mondes restent\u{a0}: Lodestone Launcher ne peut pas savoir si {version} est plus récente."
        }
        M::CopyWorldsUpgraded => {
            "Minecraft {version} met les mondes à niveau en les ouvrant, sans retour possible. Les originaux restent dans {name}."
        }
        M::CopyWorldsModsGone => {
            "Les blocs et objets des mods qui ne suivent pas sont retirés quand la copie ouvre les mondes. Les originaux restent dans {name}."
        }
        M::CopyPacks => "Packs de ressources, shaders et options",
        M::CopyMods => "Mods",
        M::CopyReading => "Lecture des mods de l’instance…",
        M::CopyNoMods => "Cette instance n’a aucun mod.",
        M::CopyModsVanilla => "Minecraft vanilla ne charge aucun mod.",
        M::CopyModsOffline => "Modrinth est injoignable, donc aucun mod ne peut être copié.",
        M::CopyModsFailed => "Impossible de vérifier les mods\u{a0}: {error}",
        M::CopyModsNoBuild => "Pas de build pour {target}, donc ceux-ci restent\u{a0}: {names}.",
        M::CopyModsNotOnModrinth => "Absents de Modrinth, donc ceux-ci restent\u{a0}: {names}.",
        M::CopyModsUnchecked => {
            "Impossible de vérifier ceux-ci sur Modrinth, donc ils restent\u{a0}: {names}."
        }
        M::CopyModsUncheckedCurseForge => {
            "Impossible de vérifier ceux-ci sur CurseForge, donc ils restent\u{a0}: {names}."
        }
        M::CopyOrUncheckMods => "ou décochez Mods pour copier sans eux.",
        M::CopySameTarget => {
            "Choisissez une autre version de Minecraft ou un autre chargeur. Dupliquer, dans Maintenance, copie l’instance telle quelle."
        }
        M::CopyAsStart => "Faire la copie",
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

        M::SharedLead => {
            "Les touches, la vidéo, le son et les autres options du jeu, identiques dans chaque instance qui les partage."
        }
        M::UseSharedSettings => "Utiliser les paramètres partagés",
        M::SharingFrom => "Reçoit les changements faits en jeu dans {name}.",
        M::SharingMain => "Les changements faits en jeu ici mettent à jour toutes les instances.",
        M::SharingNoMain => {
            "Aucune instance principale\u{a0}: les valeurs partagées ne changent que dans les Paramètres."
        }
        M::MainInstance => "Instance principale",
        M::MainInstanceHint => {
            "Ses changements faits en jeu parviennent aux autres instances qui partagent les réglages à leur prochain démarrage."
        }
        M::IsMainInstance => "C’est votre instance principale",
        M::MakeMain => "Rendre principale",
        M::StopGameToChange => {
            "Arrêtez le jeu pour modifier ceci\u{a0}: il réécrit options.txt en quittant."
        }
        M::NotShared => "Non partagé (Paramètres du launcher)",
        M::SharedKey => "Partagés",
        M::OverriddenKey => "Remplacés",
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
        M::AddModpackHint => "Rien de ce que l’instance contient n’est remplacé.",
        M::AddModpackServerHint => {
            "Pour des mods client, comme des optimisations. Les mods de {server} restent tels quels."
        }
        M::AddModpackEllipsis => "Ajouter un modpack…",
        M::RemoveAddedModpackNote => {
            "Supprime les fichiers ajoutés par le pack. Les fichiers que vous avez modifiés restent."
        }
        M::SnapshotsNotChanged => "Les instantanés n’ont pas pu être modifiés",
        M::RestoreKeptAsideTitle => "Des fichiers gênaient la restauration",
        M::SnapshotRestored => "« {name} » a été restauré.",
        M::ImportLeftOutTitle => "Certains fichiers n’ont pas été importés",
        M::ImportLeftOut => {
            "Ils n’ont pas pu être téléchargés, donc la nouvelle instance ne les a pas\u{a0}: {files}."
        }
        M::ImportPackSkippedTitle => "Il manque les fichiers du modpack",
        M::ImportPackSkipped => {
            "Le modpack du serveur n’a pas pu être téléchargé ou installé (pas de connexion, pas assez d’espace libre, ou un fichier qui n’a pas passé sa vérification), donc les fichiers que seuls ses joueurs utilisent ne sont pas dans la nouvelle instance."
        }
        M::ImportPackSkippedGone => {
            "Le modpack du serveur n’est plus sur Modrinth, donc les fichiers que seuls ses joueurs utilisent ne sont pas dans la nouvelle instance. Demandez au propriétaire du serveur de mettre à jour le modpack."
        }
        M::ImportPackSkippedCurseforge => {
            "Le serveur repose sur un modpack CurseForge, qui ne peut pas encore être installé depuis un serveur, donc les fichiers que seuls ses joueurs utilisent ne sont pas dans la nouvelle instance."
        }
        M::RestoreKeptAside => {
            "Les fichiers écrits pendant la restauration sont conservés dans {folder}."
        }
        M::ImportNoRoom => {
            "Le disque n’a pas assez de place\u{a0}: il faut {size}, dont 1\u{a0}Go de marge, et {free} sont libres."
        }
        M::ImportSpaceFree => {
            "Nécessite {size} d’espace disque, dont 1\u{a0}Go de marge\u{a0}; {free} sont libres."
        }
        M::ExportNotDownloadable => {
            "Ceux-ci ne peuvent pas être téléchargés, donc la personne qui importe le fichier ne les aura pas\u{a0}: {files}."
        }
        M::DeleteUnreadableNote => "Elles sont supprimées définitivement.",
        M::SnapshotsGroup => "Instantanés enregistrés",
        M::BackUpWorldTitle => "Sauvegarder un monde",
        M::RestoreAsNewWorld => "Restaurer comme nouveau monde",
        M::WorldBackupWorld => "Monde",
        M::WorldBackupDefaultName => "Sauvegarde de monde",
        M::WorldBackupBeforeRestoring => "Avant la restauration de «\u{a0}{name}\u{a0}»",
        M::BackUpButton => "Sauvegarder",
        M::BackingUpWorld => "Sauvegarde de {name}…",
        M::WorldBackedUp => "{name} est sauvegardé.",
        M::NoWorldsToBackUp => "Cette instance n’a pas encore de monde.",
        M::RestoreWorldTitle => "Restaurer {world}",
        M::RestoreWorldBody => {
            "{world} revient à son état dans «\u{a0}{name}\u{a0}» ({when}). Le monde tel qu’il est maintenant est d’abord enregistré dans une autre sauvegarde de monde."
        }
        M::RestoreWorldGoneBody => {
            "{world} n’est plus dans cette instance. Il revient tel qu’il était dans «\u{a0}{name}\u{a0}» ({when})."
        }
        M::SaveAndRestore => "Enregistrer et restaurer",
        M::SavingWorld => "Enregistrement du monde tel qu’il est…",
        M::UnpackingWorldBackup => "Décompression de la sauvegarde de monde…",
        M::WorldRestored => "{world} est restauré.",
        M::WorldRestoredSafety => {
            "Le monde tel qu’il était est dans la sauvegarde de monde «\u{a0}{name}\u{a0}»."
        }
        M::WorldRestoredAsNew => "Restauré comme nouveau monde, {folder}.",
        M::DeleteWorldBackupTitle => "Supprimer cette sauvegarde de monde\u{a0}?",
        M::DeleteWorldBackupBody => "«\u{a0}{name}\u{a0}» est placé dans {trash}.",
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
        P::UnreadableImages => [
            "{count}\u{a0}image illisible",
            "{count}\u{a0}images illisibles",
        ],
        P::RestoreMissing => [
            "Le stockage des images a perdu {count} de ses fichiers\u{a0}: il ne peut pas être remis en entier.",
            "Le stockage des images a perdu {count} de ses fichiers\u{a0}: il ne peut pas être remis en entier.",
        ],
        P::ImportModsWarning => [
            "Il contient {count}\u{a0}mod\u{a0}: un programme qui s’exécute au démarrage du jeu. N’importez que des fichiers de personnes de confiance.",
            "Il contient {count}\u{a0}mods\u{a0}: des programmes qui s’exécutent au démarrage du jeu. N’importez que des fichiers de personnes de confiance.",
        ],
        P::ShareToggledCount => [
            "{count} activé ou désactivé",
            "{count} activés ou désactivés",
        ],
        P::CodePackChanges => ["+ {count} modification", "+ {count} modifications"],
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
        P::CopyWorldsCount => ["{count}\u{a0}monde", "{count}\u{a0}mondes"],
        P::CopyCheckingMods => [
            "Vérification de {count}\u{a0}mod sur Modrinth…",
            "Vérification de {count}\u{a0}mods sur Modrinth…",
        ],
        P::CopyModsFound => [
            "{found} sur {count}\u{a0}mod a un build pour {target}.",
            "{found} sur {count}\u{a0}mods ont un build pour {target}.",
        ],
        P::RestorePutsBack => [
            "Remet {count}\u{a0}fichier\u{a0}: {names}",
            "Remet {count}\u{a0}fichiers\u{a0}: {names}",
        ],
        P::RestoreRemoves => [
            "Supprime {count}\u{a0}fichier\u{a0}: {names}",
            "Supprime {count}\u{a0}fichiers\u{a0}: {names}",
        ],
        P::RestoreChanges => [
            "Modifie {count}\u{a0}fichier\u{a0}: {names}",
            "Modifie {count}\u{a0}fichiers\u{a0}: {names}",
        ],
        P::ImportCarriedMods => [
            "{count} d’entre eux est dans le fichier lui-même, et non téléchargé depuis Modrinth ou CurseForge\u{a0}: {names}",
            "{count} d’entre eux sont dans le fichier lui-même, et non téléchargés depuis Modrinth ou CurseForge\u{a0}: {names}",
        ],
        P::ImportFromServer => [
            "Une sauvegarde du serveur {server}\u{a0}: son monde, avec les mods que reçoivent ses joueurs. {count} fichier du serveur n’est pas repris (paramètres, opérateurs et plugins du serveur).",
            "Une sauvegarde du serveur {server}\u{a0}: son monde, avec les mods que reçoivent ses joueurs. {count} fichiers du serveur ne sont pas repris (paramètres, opérateurs et plugins du serveur).",
        ],
        P::WorldBackupLeftOut => [
            "{count}\u{a0}fichier laissé de côté",
            "{count}\u{a0}fichiers laissés de côté",
        ],
    }
}
