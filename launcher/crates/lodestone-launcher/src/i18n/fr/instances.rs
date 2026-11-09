use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::TwoSentences => "{first} {second}",
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
        M::ProgressDetail => "{files_done}/{files_total} · {done}\u{a0}Mo/{total}\u{a0}Mo",

        M::PreparingNote => "Préparation de Minecraft {version} ({loader})",
        M::JoiningNote => "Connexion à {address} dès que le jeu aura démarré",
        M::VerifyingNote => "Vérification des fichiers du jeu de Minecraft {version} ({loader})",
        M::RosettaNote => "Cette version s’exécute dans un Java Intel (x86_64) via Rosetta 2.",
        M::StartingJavaNote => "Démarrage de Java\u{a0}: {command}",
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
        M::CouldntStartMinecraft => "Impossible de démarrer Minecraft",
        M::CouldntStartJava => "Impossible de démarrer Java",
        M::CouldntVerify => "Impossible de vérifier les fichiers du jeu",
        M::GameFilesVerified => "Fichiers du jeu vérifiés",
        M::EveryFileIntact => "Tous les fichiers étaient intacts. {summary}",
        M::ModpackUpdating => "Le modpack est en cours de mise à jour",
        M::WaitForUpdate => "Attendez la fin de la mise à jour, puis cliquez sur Jouer.",
        M::StopBeforeDeleting => "Arrêtez le jeu avant de supprimer son instance.",
        M::CouldntSavePlayed => "Impossible d’enregistrer la date de la partie\u{a0}: {error}",
        M::CouldntSavePlayTime => "Impossible d’enregistrer le temps de jeu\u{a0}: {error}",
        M::ModpackExported => "Modpack exporté",
        M::CouldntExportModpack => "Impossible d’exporter le modpack",
        M::CouldntSwitchLoader => "Impossible de passer à un chargeur de mods",
        M::NoModLoaderFor => {
            "Aucun chargeur de mods pris en charge par Lodestone Launcher ne fonctionne avec Minecraft {version}\u{a0}: Fabric commence à la 1.14, et Forge n’a pas de version pour celle-ci."
        }
        M::SwitchedTo => "Passage à {loader} effectué",
        M::SwitchedToMessage => {
            "{name} utilise maintenant {loader} {version}. Il sera installé la prochaine fois que vous cliquerez sur Jouer."
        }

        M::SearchLog => "Rechercher dans le journal",
        M::LevelDebug => "Débogage",
        M::LevelInfo => "Info",
        M::LevelWarn => "Avert.",
        M::LevelError => "Erreur",
        M::LogEmpty => "La sortie du jeu s’affiche ici quand vous cliquez sur Jouer.",
        M::NoLinesMatch => "Aucune ligne ne correspond au filtre.",
        M::JumpToLatest => "Aller à la fin",
        M::CopyLog => "Copier le journal",

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
        M::StopAndClose => "Arrêter et fermer",

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
        M::CopyingConfigs => "Copie des configurations du pack",
        M::RecordingFiles => "Enregistrement des fichiers du pack",
        M::SavingSnapshot => "Sauvegarde des fichiers de l’instance",
        M::BackingUp => "Sauvegarde de {name}",
        M::UpdatingTo => "Mise à jour vers {version}",

        M::SearchModpacks => "Rechercher un modpack sur Modrinth ou coller un lien",
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
    }
}
