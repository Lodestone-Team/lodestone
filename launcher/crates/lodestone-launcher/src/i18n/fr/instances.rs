use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::TwoSentences => "{first} {second}",
        M::ShareCodeReady => "{name} est prête",
        M::ShareCodeNotInstalled => "Non installés\u{a0}: {names}.",
        M::ShareCodeContentFailed => "Impossible d’installer le contenu du code de partage",
        M::ShareCodeOffline => {
            "Modrinth est injoignable\u{a0}: l’instance a été créée sans son contenu."
        }
        M::TitleAndMessage => "{title}\u{a0}: {message}",
        M::Vanilla => "Vanilla",

        M::StagePreparing => "Préparation",
        M::StageJava => "Téléchargement de Java",
        M::StageLibraries => "Bibliothèques du jeu",
        M::StageLoader => "Installation du chargeur",
        M::StageProcessors => "Adaptation au chargeur",
        M::StageAssets => "Ressources du jeu",
        M::StageNatives => "Bibliothèques natives",
        M::StageStarting => "Démarrage de Java",
        M::StageLaunching => "Lancement de Minecraft",
        M::ProgressDetail => "{files_done}/{files_total} · {done}\u{a0}Mo/{total}\u{a0}Mo",

        M::PreparingNote => "Préparation de Minecraft {version} ({loader})",
        M::JoiningNote => "Connexion à {address} dès que le jeu aura démarré",
        M::VerifyingNote => "Vérification des fichiers du jeu de Minecraft {version} ({loader})",
        M::RosettaNote => "Le jeu s’exécute dans un Java Intel (x86_64) via Rosetta 2.",
        M::StartingJavaNote => "Démarrage de Java\u{a0}: {command}",
        M::AutoMemoryNote => {
            "Configuration automatique\u{a0}: mémoire réglée sur {memory} pour cet ordinateur et les mods de l’instance"
        }
        M::AutoMemoryLimitedNote => {
            "Les mods voudraient {wanted} de mémoire, plus que cet ordinateur ne peut en céder\u{a0}; fermez les autres applications pendant que vous jouez."
        }
        M::AutoJvmArgsNote => {
            "Configuration automatique\u{a0}: ramasse-miettes de Java réglé pour le jeu moddé"
        }
        M::AutoLowMemoryNote => {
            "Cette instance dispose de {have} de mémoire\u{a0}; avec ses mods, {recommended} est recommandé. Modifiez-la dans les paramètres de l’instance."
        }
        M::AutoVideoNote => {
            "Configuration automatique\u{a0}: réglages graphiques choisis pour cet ordinateur (distance d’affichage {distance})"
        }
        M::AutoVideoNoDistanceNote => {
            "Configuration automatique\u{a0}: réglages graphiques choisis pour cet ordinateur"
        }
        M::AutoControlsNote => {
            "Configuration automatique\u{a0}: sensibilité de la souris, touches et langue reprises de {source}"
        }
        M::AutoControlsLauncherNote => {
            "Configuration automatique\u{a0}: sensibilité de la souris, touches et langue reprises du Minecraft Launcher"
        }
        M::AutoControlsOtherLauncherNote => {
            "Configuration automatique\u{a0}: sensibilité de la souris, touches et langue reprises de {instance} dans {launcher}"
        }
        M::StoppedNote => "Arrêté",
        M::ExitedWithCode => "Minecraft s’est fermé avec le code {code}",
        M::EndedBySignal => "Minecraft a été arrêté par un signal",
        M::CancellingNote => "Annulation…",
        M::StoppingNote => "Arrêt de Minecraft",
        M::Cancelled => "Annulé",

        M::CouldntSignIn => "Impossible de se connecter",
        M::RosettaNeeded => "Rosetta 2 est nécessaire",
        M::RosettaMissing => {
            "Cette version de Minecraft a besoin d’un Java Intel (x86_64), qui s’exécute sur Apple Silicon grâce à Rosetta 2. Rosetta n’est pas installé. Installez-le en exécutant cette commande dans le Terminal, puis cliquez de nouveau sur Jouer\u{a0}:\n\nsoftwareupdate --install-rosetta"
        }
        M::RosettaNoticeTitle => "Minecraft {version} s’exécute via Rosetta\u{a0}2",
        M::RosettaNoticeBody => {
            "Mojang n’a pas fait de version Apple Silicon de certaines bibliothèques dont cette version a besoin\u{a0}: elle s’exécute donc dans un Java Intel grâce à Rosetta\u{a0}2. Elle met plus de temps à démarrer, puis fonctionne normalement. Minecraft 1.14 et les versions suivantes s’exécutent nativement."
        }
        M::RosettaNoticeBodyOverride => {
            "Cette instance utilise un Java Intel, choisi dans ses Paramètres\u{a0}: le jeu s’exécute donc grâce à Rosetta\u{a0}2. Il met plus de temps à démarrer, puis fonctionne normalement. Pour l’exécuter nativement, choisissez un Java fait pour Apple Silicon, ou revenez au Java par défaut."
        }
        M::NoNativeJava8 => {
            "Cette version de Minecraft a besoin de Java\u{a0}8, et aucun Java\u{a0}8 pour Apple Silicon n’est disponible pour le moment\u{a0}: Mojang n’en fait pas, et Azul n’en propose aucun. Réessayez plus tard, ou choisissez un Java\u{a0}8 fait pour Apple Silicon (arm64) dans les Paramètres de l’instance."
        }
        M::NeedsIntelJava => {
            "Sur Apple Silicon, Minecraft {version} ne s’exécute que dans un Java Intel (x86_64), grâce à Rosetta\u{a0}2, et le Java choisi dans les Paramètres de l’instance est fait pour Apple Silicon. Choisissez-y un Java Intel, ou le Java de Mojang."
        }
        M::CouldntStartMinecraft => "Impossible de démarrer Minecraft",
        M::CouldntStartJava => "Impossible de démarrer Java",
        M::CouldntVerify => "Impossible de vérifier les fichiers du jeu",
        M::GameFilesVerified => "Fichiers du jeu vérifiés",
        M::EveryFileIntact => "Tous les fichiers étaient intacts. {summary}",
        M::ModpackUpdating => "Le modpack est en cours de mise à jour",
        M::WaitForUpdate => "Attendez la fin de la mise à jour, puis cliquez sur Jouer.",
        M::StopBeforeDeleting => "Arrêtez le jeu avant de supprimer son instance.",
        M::FilesBusy => {
            "Attendez que les fichiers de l’instance aient fini de changer, puis réessayez."
        }
        M::CouldntSavePlayed => "Impossible d’enregistrer la date de la partie\u{a0}: {error}",
        M::CouldntSavePlayTime => "Impossible d’enregistrer le temps de jeu\u{a0}: {error}",
        M::ModpackExported => "Modpack exporté",
        M::CouldntExportModpack => "Impossible d’exporter le modpack",

        M::SearchLog => "Rechercher dans le journal",
        M::LevelDebug => "Débogage",
        M::LevelInfo => "Info",
        M::LevelWarn => "Avert.",
        M::LevelError => "Erreur",
        M::LogEmpty => "La sortie du jeu s’affiche ici quand vous cliquez sur Jouer.",
        M::NoLinesMatch => "Aucune ligne ne correspond au filtre.",
        M::JumpToLatest => "Aller à la fin",
        M::CopyLog => "Copier le journal",
        M::LogSource => "Journal à afficher",
        M::LogThisSession => "Cette session",
        M::LogNoFiles => "Aucun journal ni rapport de plantage antérieur pour l’instant.",
        M::LogKindLog => "Journal",
        M::LogKindCrash => "Rapport de plantage",
        M::LogKindJvm => "Plantage de Java",
        M::LogReading => "Lecture de {name}",
        M::LogReadFailed => "Impossible de lire {name} : {error}",
        M::LogFileEmpty => "Ce fichier est vide.",

        M::ImportTitle => "Importer depuis un autre launcher",
        M::ImportFromHere => "Importer depuis ce dossier",
        M::LinksLeftOut => "Les liens vers l’extérieur du dossier du jeu ont été ignorés.",
        M::NewestRelease => "La version stable la plus récente",
        M::NewestSnapshot => "Le snapshot le plus récent",
        M::NoInstances => "Aucune instance.",
        M::LookingForLaunchers => "Recherche des autres launchers",
        M::NoLaunchersFound => {
            "Aucun autre launcher n’a été trouvé aux emplacements habituels. Choisissez le dossier d’un launcher, ou celui d’une instance, pour importer depuis celui-ci."
        }
        M::Waiting => "En attente",
        M::FindingVersion => "Recherche de la version et du chargeur",
        M::Imported => "Importé",
        M::ImportedNote => "Importé. {note}",
        M::Copying => "Copie\u{a0}: {done} sur {total}",
        M::MatchingModrinth => "Association des mods et des packs à Modrinth",
        M::CheckToImport => {
            "Cochez les instances à importer. Leurs fichiers sont copiés\u{202F}; l’autre launcher garde les siens."
        }
        M::LeaveWorldsOut => "Ne pas copier les mondes",
        M::LeaveWorldsOutSize => "Ne pas copier les mondes ({size})",
        M::Import => "Importer",
        M::ChooseFolder => "Choisir un dossier…",
        M::StopImportingTitle => "Arrêter l’importation\u{202F}?",
        M::StopImportingBody => {
            "Les importations en cours s’arrêtent, et ce qu’elles ont déjà copié est supprimé. Les instances déjà importées restent."
        }
        M::StopImporting => "Arrêter l’importation",
        M::KeepImporting => "Continuer l’importation",

        M::NotAModpack => "Ce n’est pas un modpack",
        M::NotMrpack => {
            "{name} n’est pas un modpack. Les modpacks Modrinth se terminent par .mrpack, et ceux de CurseForge sont des fichiers .zip contenant un manifest.json."
        }
        M::CurseForgeUnreachable => {
            "C’est un modpack CurseForge, et cette copie de Lodestone Launcher ne peut pas joindre CurseForge."
        }
        M::CurseForgePackOnWebsite => {
            "Ce modpack ne peut être téléchargé que depuis le site de CurseForge. Téléchargez-le là-bas, puis déposez le .zip sur Lodestone Launcher."
        }
        M::Modpack => "Modpack",
        M::ReadingModpack => "Lecture du modpack",
        M::ModpackReady => "L’instance {name} est prête",
        M::ModpackReadyMessage => {
            "Minecraft {version} avec {loader}. Cliquez sur Jouer pour télécharger le jeu et le démarrer."
        }
        M::CouldntImportModpack => "Impossible d’importer le modpack",
        M::CouldntUpdateModpack => "Impossible de mettre à jour le modpack",
        M::StopGameFirst => "Arrêtez d’abord le jeu.",
        M::ModpackUpdated => "Modpack mis à jour",
        M::ModpackUpdatedMessage => {
            "{name} utilise maintenant {pack} {version}. Les fichiers remplacés sont conservés dans {path}."
        }
        M::UpdatedButNotSaved => {
            "Les fichiers ont été mis à jour, mais l’instance n’a pas pu être enregistrée\u{a0}: {error}"
        }
        M::DownloadingName => "Téléchargement de {name}",
        M::InstallingName => "Installation de {name}",
        M::CopyReady => "L’instance {name} est prête",
        M::CouldntCopy => "Impossible de faire la copie",
        M::CopyingWorlds => "Copie des mondes",
        M::CopyingPacksAndOptions => "Copie des packs de ressources, des shaders et des options",
        M::InstallingMods => "Installation des mods",
        M::CopyingConfigs => "Copie des configurations du pack",
        M::RecordingFiles => "Enregistrement des fichiers du pack",
        M::SavingSnapshot => "Sauvegarde des fichiers de l’instance",
        M::BackingUp => "Sauvegarde de {name}",
        M::UpdatingTo => "Mise à jour vers {version}",

        M::SearchModpacks => "Rechercher un modpack sur Modrinth ou coller un lien",
        M::SearchModpacksCurseForge => "Rechercher un modpack sur CurseForge",
        M::SortRelevance => "Pertinence",
        M::SortDownloads => "Téléchargements",
        M::SortFollows => "Abonnés",
        M::SortUpdated => "Mise à jour",
        M::LinkNotModpack => {
            "Ce lien mène à un {kind}, pas à un modpack. Ajoutez-le depuis l’onglet Contenu d’une instance."
        }
        M::KindMod => "mod",
        M::KindResourcePack => "pack de ressources",
        M::KindShaderPack => "pack de shaders",
        M::KindDataPack => "pack de données",
        M::KindPlugin => "plugin",
        M::SearchingModrinth => "Recherche sur Modrinth",
        M::NoModpacksMatch => "Aucun modpack ne correspond.",
        M::ByAuthor => "par {author} · {downloads}\u{a0}téléchargements",
        M::PickModpack => "Choisissez un modpack pour voir ses versions.",
        M::LoadingModpack => "Chargement du modpack",
        M::Versions => "Versions",
        M::Description => "Description",
        M::NoVersions => "Ce modpack n’a aucune version.",
        M::LoadingVersions => "Chargement des versions",
        M::FindModButton => "Le trouver pour moi",
        M::FinderTitle => "Trouver le mod qui fait planter {name}",
        M::FinderIntro => {
            "Lodestone Launcher va lancer le jeu plusieurs fois, en désactivant à chaque fois la moitié des mods qui pourraient causer le plantage, jusqu’à ce qu’il n’en reste qu’un. Ensuite, vos mods reviennent comme avant."
        }
        M::FinderAutomatic => {
            "Le jeu a planté au démarrage, vous n’avez donc rien à faire. Chaque essai se termine quand le jeu plante ou arrive au menu principal."
        }
        M::FinderManual => {
            "Le jeu a planté pendant que vous jouiez, vous devrez donc jouer chaque essai. Refaites ce qui l’avait fait planter. S’il plante encore, l’essai suivant démarre tout seul. Sinon, appuyez sur Il n’a pas planté."
        }
        M::FinderByHand => {
            "Jouez chaque essai et refaites ce qui a fait planter le jeu. S’il plante, l’essai suivant démarre tout seul. Sinon, appuyez sur Il n’a pas planté."
        }
        M::FinderStart => "Commencer",
        M::FinderReading => "Vérification de vos mods",
        M::FinderTry => "Essai {number}",
        M::FinderStarting => "Lancement du jeu",
        M::FinderWaiting => "En attente d’un éventuel plantage",
        M::FinderSettling => "Le menu principal est atteint. Vérification que le jeu reste ouvert",
        M::FinderPlay => {
            "Jouez jusqu’au plantage. S’il ne plante pas, appuyez sur Il n’a pas planté."
        }
        M::FinderAsk => "Le jeu s’est fermé. Le problème est-il survenu\u{202F}?",
        M::FinderDidntCrash => "Il n’a pas planté",
        M::FinderItCrashed => "Il a planté",
        M::FinderStopping => "Arrêt du jeu",
        M::FinderFound => "Trouvé",
        M::FinderFoundOne => "Le jeu ne plante pas quand {name} est désactivé.",
        M::FinderFoundGroup => {
            "Le jeu ne plante pas quand ceux-ci sont désactivés\u{a0}: {names}. Ils dépendent les uns des autres, donc ils ne peuvent être désactivés qu’ensemble."
        }
        M::FinderAlsoOff => "Les mods qui en ont besoin restent aussi désactivés\u{a0}: {names}.",
        M::FinderKeepOff => "Le garder désactivé",
        M::FinderPutBack => "Tout réactiver",
        M::FinderNotFound => {
            "Le jeu a continué à planter même avec ces mods désactivés, donc la cause n’est sans doute pas un mod, ou le plantage n’arrive que de temps en temps. Vos mods sont revenus comme avant."
        }
        M::FinderRefused => {
            "Certains mods ne se chargeaient pas sans ceux qui avaient été désactivés, la recherche a donc dû s’arrêter. Vos mods sont revenus comme avant."
        }
        M::FinderFailed => {
            "Un problème est survenu et la recherche s’est arrêtée. Vos mods sont revenus comme avant."
        }
        M::FinderStopped => "Recherche arrêtée. Vos mods sont revenus comme avant.",
        M::FinderStop => "Arrêter",
        M::FinderInterruptedTitle => "Terminer la recherche du mod dans {name}\u{202F}?",
        M::FinderInterrupted => {
            "Lodestone Launcher s’est fermé au milieu d’une recherche, donc certains mods de {name} sont encore désactivés."
        }
        M::FinderPutBackFailed => "Certains mods n’ont pas pu être réactivés\u{a0}: {error}",

        M::AddingName => "Ajout de {name}",
        M::ModrinthUnreachable => "Modrinth est injoignable.",
        M::ModpackAddedAlready => {
            "{pack} est déjà ajouté à cette instance. Retirez-le d’abord dans les Paramètres pour en ajouter une autre version."
        }
        M::CouldntAddModpack => "Impossible d’ajouter le modpack",
        M::ModpackAdded => "Modpack ajouté",
        M::ModpackAddedMessage => "{pack} a été ajouté à {name}. Appuyez sur Jouer pour la lancer.",
        M::CouldntRemoveModpack => "Impossible de retirer le modpack",
        M::ModpackRemoved => "Modpack retiré",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::CheckSummary => [
            "{count} fichier du jeu vérifié en {ms}\u{a0}ms (inchangés depuis la dernière vérification\u{a0}: {unchanged}, hachés\u{a0}: {hashed}, téléchargés\u{a0}: {downloaded}).",
            "{count} fichiers du jeu vérifiés en {ms}\u{a0}ms (inchangés depuis la dernière vérification\u{a0}: {unchanged}, hachés\u{a0}: {hashed}, téléchargés\u{a0}: {downloaded}).",
        ],
        P::FilesRepaired => [
            "{count} fichier manquant ou endommagé a été téléchargé de nouveau. {summary}",
            "{count} fichiers manquants ou endommagés ont été téléchargés de nouveau. {summary}",
        ],
        P::BrokenFolders => [
            "{count} dossier d’instance n’a pas pu être lu\u{a0}: {path}.",
            "{count} dossiers d’instance n’ont pas pu être lus, par exemple {path}.",
        ],
        P::FilesSaved => [
            "{count} fichier enregistré dans {path}.",
            "{count} fichiers enregistrés dans {path}.",
        ],
        P::CurseForgeLeftOut => [
            "{count} fichier a été ignoré car il n’est que sur CurseForge, dont les fichiers ne peuvent pas aller dans un pack Modrinth\u{a0}: {names}.",
            "{count} fichiers ont été ignorés car ils ne sont que sur CurseForge, dont les fichiers ne peuvent pas aller dans un pack Modrinth\u{a0}: {names}.",
        ],
        P::FilesLeftOut => [
            "{count} fichier a été ignoré\u{a0}: {name}.",
            "{count} fichiers ont été ignorés, par exemple {name}.",
        ],
        P::LogLinesSkipped => [
            "{count} ligne au début n’est pas affichée",
            "les {count} premières lignes ne sont pas affichées",
        ],
        P::LinesShown => ["{shown} ligne sur {count}", "{shown} lignes sur {count}"],
        P::LinesShownFollowing => [
            "{shown} ligne sur {count} · suivi en direct",
            "{shown} lignes sur {count} · suivi en direct",
        ],
        P::ProblemAndMore => [
            "{problem} (et {count} autre)",
            "{problem} (et {count} autres)",
        ],
        P::InstancesToCopy => [
            "{count} instance à copier, {size}.",
            "{count} instances à copier, {size}.",
        ],
        P::ImportInstances => ["Importer {count} instance", "Importer {count} instances"],
        P::ShareCodeInstalled => [
            "{count} élément installé depuis le code.",
            "{count} éléments installés depuis le code.",
        ],
        P::PackFilesToDownload => [
            "{count} fichier doit être téléchargé depuis le site de CurseForge\u{a0}: l’onglet Contenu de l’instance l’affiche.",
            "{count} fichiers doivent être téléchargés depuis le site de CurseForge\u{a0}: l’onglet Contenu de l’instance les affiche.",
        ],
        P::PackFilesLeftOut => [
            "{count} fichier du pack a été ignoré\u{a0}: {path}.",
            "{count} fichiers du pack ont été ignorés, par exemple {path}.",
        ],
        P::WorldsBackedUp => [
            "{count} monde a d’abord été sauvegardé.",
            "{count} mondes ont d’abord été sauvegardés.",
        ],
        P::DownloadingFiles => [
            "Téléchargement de {count} fichier",
            "Téléchargement de {count} fichiers",
        ],
        P::CopyModsLeftBehind => [
            "{count}\u{a0}mod n’a pas été copié\u{a0}: {names}.",
            "{count}\u{a0}mods n’ont pas été copiés\u{a0}: {names}.",
        ],
        P::FinderModsLeft => [
            "{count} mod reste à vérifier",
            "{count} mods restent à vérifier",
        ],
        P::FinderModsOff => ["{count} mod désactivé", "{count} mods désactivés"],
        P::FinderTriesLeft => [
            "Environ {count} essai de plus",
            "Environ {count} essais de plus",
        ],

        P::AddedPackFilesRemoved => [
            "{count} fichier de {pack} a été supprimé.",
            "{count} fichiers de {pack} ont été supprimés.",
        ],
        P::AddedPackFilesKept => [
            "{count} fichier que vous avez modifié a été gardé.",
            "{count} fichiers que vous avez modifiés ont été gardés.",
        ],
    }
}
