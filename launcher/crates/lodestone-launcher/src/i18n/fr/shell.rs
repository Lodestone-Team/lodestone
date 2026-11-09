use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "Quitter {app}",
        M::Language => "Langue",
        M::LanguageSystem => "Système",
        M::LanguageSystemIs => "Système ({language})",
        M::Zoom => "Zoom",
        M::ViewMenu => "Présentation",
        M::ZoomIn => "Zoom avant",
        M::ZoomOut => "Zoom arrière",
        M::ActualSize => "Taille réelle",
        M::EditMenu => "Édition",
        M::Undo => "Annuler",
        M::Redo => "Rétablir",
        M::Undid => "Annulé\u{a0}: {change}",
        M::Redid => "Rétabli\u{a0}: {change}",
        M::ChangeTo => "{setting} → {value}",
        M::SettingsOf => "Paramètres de {name}",

        M::Play => "Jouer",
        M::Playing => "En jeu",
        M::Launching => "Lancement",
        M::TabContent => "Contenu",
        M::TabWorlds => "Mondes",
        M::Settings => "Paramètres",
        M::TabLogs => "Journaux",

        M::LastPlayed => "Dernière partie",
        M::Instances => "Instances",
        M::NoInstancesYet => "Aucune instance pour l’instant.",
        M::NewInstance => "Nouvelle instance…",
        M::ImportInstances => "Importer…",
        M::Servers => "Serveurs",
        M::AppVersion => "Lodestone Launcher v{version}",
        M::PlayedInTotal => "{when} · {time} au total",
        M::ImportProgress => "{percent} · {label}",
        M::GroupModded => "{loader} (moddé)",
        M::Vanilla => "Vanilla",
        M::GroupImported => "Importées",

        M::NoInstanceSelected => "Aucune instance sélectionnée",
        M::NoInstanceSelectedHint => {
            "Créez une instance pour choisir une version de Minecraft et un chargeur."
        }
        M::MinecraftVersion => "Minecraft {version}",
        M::MinecraftWith => "Minecraft {version} avec {loader}",
        M::HeroPlayed => "{game}. Temps de jeu\u{a0}: {time}.",
        M::HeroNotPlayed => "{game}. Appuyez sur Jouer pour la télécharger et lancer le jeu.",
        M::CreateFirstInstance => "Créez votre première instance",
        M::InstanceExplainer => {
            "Une instance est une version de Minecraft avec ses propres mods, mondes et paramètres."
        }
        M::ContentPlaceholder => "Le contenu s’affiche ici une fois que vous avez une instance.",
        M::WorldsPlaceholder => "Les mondes s’affichent ici une fois que vous avez une instance.",
        M::SettingsPlaceholder => {
            "Les paramètres s’affichent ici une fois que vous avez une instance."
        }
        M::LogsPlaceholder => "Les journaux s’affichent ici une fois que vous avez une instance.",
        M::LinkedTo => "Liée à {server} sur {core}",
        M::LinkedToRemoved => {
            "Liée à {server} sur {core} (ce core a été retiré de la page Serveurs)"
        }
        M::PickerInstance => "Instance",
        M::PickerVersion => "Version",
        M::NoInstances => "Aucune instance",
        M::AndJoin => "et rejoindre {server}",
        M::PlayWithoutJoining => "Jouer sans rejoindre",

        M::DeleteNamed => "Supprimer {name}\u{202F}?",
        M::DeleteWorldBody => {
            "Le dossier du monde est placé dans {trash}, d’où vous pouvez le récupérer. Ses sauvegardes sont conservées."
        }
        M::DeleteInstanceBody => {
            "Le dossier de l’instance, avec ses mondes, ses mods et ses paramètres, est placé dans {trash}. Vous pouvez le récupérer depuis cet endroit."
        }
        M::ReplaceWorldTitle => "Remplacer {name} par cette sauvegarde\u{202F}?",
        M::ReplaceWorldBody => {
            "{name} revient à son état de la sauvegarde ({when}). Tout ce qui a changé depuis est perdu dans le monde, mais il est d’abord sauvegardé et le dossier actuel est placé dans {trash}."
        }
        M::BackUpAndReplace => "Sauvegarder et remplacer",
        M::SwitchToLoader => "Passer à {loader}\u{202F}?",
        M::SwitchToVanilla => "Passer à Vanilla\u{202F}?",
        M::ModsWontLoadOn => {
            "Ces mods activés sont faits pour un autre chargeur et ne se chargeront pas avec {loader}\u{a0}:"
        }
        M::ModsWontLoadVanilla => {
            "Minecraft Vanilla ne charge aucun mod, donc ces mods activés ne se chargeront pas\u{a0}:"
        }
        M::ModMadeFor => "mod {loader}",
        M::TurningOffRenames => {
            "Les désactiver les renomme en .disabled, pour que vous puissiez les réactiver dans Contenu."
        }
        M::WorldsAtRisk => {
            "Les mondes qui utilisaient des blocs ou des objets de ces mods peuvent les perdre. Sauvegardez d’abord vos mondes."
        }
        M::TurnOffAndSave => "Les désactiver et enregistrer",
        M::SaveAndKeep => "Enregistrer et les garder",
        M::RemoveAccountTitle => "Retirer {name}\u{202F}?",
        M::RemoveAccountBody => {
            "Lodestone Launcher oublie ce compte et supprime sa connexion enregistrée de votre {keychain}. Vous pourrez l’ajouter de nouveau plus tard."
        }
        M::RemoveAccount => "Retirer le compte",
        M::MinecraftCrashed => "Minecraft a planté\u{a0}: {description}.",
        M::MinecraftExitCode => "Minecraft s’est arrêté avec le code de sortie {code}.",
        M::MinecraftStopped => "Minecraft s’est arrêté de façon inattendue.",
        M::CrashTitle => "{name} a planté",
        M::LikelyCause => "Cause probable",
        M::OpenCrashReport => "Ouvrir le rapport de plantage",
        M::CauseWrongJava => {
            "Mauvaise version de Java. Cette version de Minecraft (ou un mod) a besoin d’un autre Java. Supprimez le Java personnalisé dans Paramètres pour utiliser celui que fournit Mojang."
        }
        M::CauseOutOfMemory => {
            "Mémoire insuffisante. Augmentez la mémoire maximale dans Paramètres, ou fermez d’autres applications."
        }
        M::CauseNatives => {
            "Des bibliothèques natives manquent ou sont faites pour un autre processeur. Sur Apple Silicon, les versions antérieures à 1.19 tournent dans un Java Intel sous Rosetta\u{a0}2\u{202F}; vérifiez que Rosetta est installé et que le Java personnalisé n’est pas un Java arm64."
        }
        M::CauseJvmOptions => {
            "Java a refusé l’un des réglages. Vérifiez les arguments JVM et la mémoire dans Paramètres."
        }
        M::CauseGraphics => {
            if cfg!(target_os = "macos") {
                "Le jeu n’a pas pu créer sa fenêtre ou initialiser OpenGL. Mettez à jour macOS, ou essayez sans taille de fenêtre personnalisée ni plein écran."
            } else {
                "Le jeu n’a pas pu créer sa fenêtre ou initialiser OpenGL. Mettez à jour vos pilotes graphiques, ou essayez sans taille de fenêtre personnalisée ni plein écran."
            }
        }
        M::CauseModPatch => {
            "Un mod n’a pas pu appliquer ses modifications au jeu. Il est sans doute fait pour une autre version de Minecraft, ou il a besoin d’une version plus récente d’un autre mod. Mettez-le à jour ou désactivez-le."
        }
        M::ProbablyCausedBy => "Probablement causé par",
        M::ModsInvolved => "Mods en cause",
        M::ModsInvolvedHint => {
            "Chacun d’eux pourrait être en cause. Essayez de les désactiver un par un."
        }
        M::EvidenceFailedToPatch => {
            "Il n’a pas pu se charger, il est peut-être fait pour une autre version de Minecraft."
        }
        M::EvidenceCrashedInItsCode => "Le plantage s’est produit dans ce mod.",
        M::EvidencePatchedCrashingCode => "Il modifie la partie du jeu qui a planté.",
        M::EvidenceInStack => "Il était en cours d’exécution quand le jeu a planté.",
        M::TurnOffMod => "Désactiver",
        M::TurnOnMod => "Réactiver",
        M::ModTurnedOff => {
            "Désactivé. Rejouez pour voir si le problème est réglé. Vous pouvez le réactiver ici ou dans l’onglet Contenu."
        }
        M::CopyLog => "Copier le journal",

        M::UpdateTitle => "Mettre à jour {name}\u{202F}?",
        M::ModpackSummary => {
            "{pack} {from} → {to}. Minecraft {game} avec {loader}. {size} à télécharger."
        }
        M::NoFilesChange => "Aucun fichier ne change\u{202F}; seule la version est enregistrée.",
        M::UpdatedFiles => "Mis à jour ({count})",
        M::AddedFiles => "Ajoutés ({count})",
        M::RemovedFiles => "Supprimés ({count})",
        M::ReplacedOwnFiles => "Vos fichiers désormais fournis par le pack ({count})",
        M::KeptFiles => "Gardés tels que vous les avez modifiés ({count})",
        M::WrittenFiles => "Configurations et autres fichiers écrits ({count})",
        M::ModpackUpdateSafety => {
            "Vos propres mods, vos mondes et vos configurations modifiées ne sont pas touchés. Chaque fichier remplacé ou supprimé est d’abord enregistré dans le dossier de sauvegardes de l’instance."
        }
        M::BackUpEveryWorld => "Sauvegarder d’abord tous les mondes",

        M::SaveChangesTitle => "Enregistrer vos modifications\u{202F}?",
        M::UnsavedDefaultsBody => {
            "Vous avez modifié les paramètres par défaut des instances. Les enregistrer ou les abandonner\u{202F}?"
        }
        M::UnsavedInstanceBody => {
            "Vous avez modifié les paramètres de {name}. Les enregistrer ou les abandonner\u{202F}?"
        }
        M::UnsavedThisInstanceBody => {
            "Vous avez modifié les paramètres de cette instance. Les enregistrer ou les abandonner\u{202F}?"
        }
        M::UnsavedInDefaults => {
            "Vous avez des modifications non enregistrées dans les paramètres par défaut des instances"
        }

        M::GroupLauncher => "Lanceur",
        M::InstanceDefaults => "Paramètres par défaut des instances",
        M::SharedGameSettings => "Paramètres de jeu partagés",
        M::General => "Général",
        M::Accounts => "Comptes",
        M::Developer => "Développeur",
        M::DefaultsLead => "Chaque instance peut les modifier dans ses propres paramètres.",
        M::SharedLead => "Gardez vos réglages synchronisés entre vos instances.",
        M::NoAccount => "Aucun compte",
        M::ManageAccounts => "Gérer les comptes",
        M::GroupAutomatically => "Grouper les instances automatiquement",
        M::GroupAutomaticallyHint => "Classe les instances sans groupe par chargeur de mods.",
        M::WhenAGameStarts => "Au lancement d’un jeu",
        M::AfterLaunchStay => "Rester ouvert",
        M::AfterLaunchMinimize => "Réduire",
        M::AfterLaunchQuit => "Quitter",
        M::AfterLaunchStayHint => "Le lanceur reste ouvert à côté du jeu.",
        M::AfterLaunchMinimizeHint => {
            "Le lanceur se réduit dès que la fenêtre du jeu s’ouvre, et revient quand le jeu se ferme."
        }
        M::AfterLaunchQuitHint => {
            "Le lanceur se ferme dès que la fenêtre du jeu s’ouvre. Le jeu continue, mais son journal, son rapport de plantage et son temps de jeu ne sont pas enregistrés."
        }
        M::OfflineAccount => "Compte hors ligne",
        M::MicrosoftAccount => "Compte Microsoft",
        M::SignInAgain => "Se reconnecter",
        M::InUse => "Utilisé",
        M::Use => "Utiliser",
        M::AddAnAccount => "Ajouter un compte",
        M::AddMicrosoftEllipsis => "Microsoft…",
        M::AddOfflineEllipsis => "Hors ligne…",

        M::Memory => "Mémoire",
        M::MemoryError => "Mémoire\u{a0}: {error}",
        M::MemoryDefaultHint => "Par défaut\u{a0}: {min} – {max}",
        M::MegabytesUnit => "Mo",
        M::Window => "Fenêtre",
        M::Fullscreen => "Plein écran",
        M::JvmArguments => "Arguments JVM",
        M::GameArguments => "Arguments du jeu",
        M::EnvironmentVariables => "Variables d’environnement",
        M::SyncedFrom => "{name} est maintenant votre instance principale.",
        M::SyncedFromNone => {
            "Plus d’instance principale\u{a0}: modifiez les réglages partagés ici."
        }
        M::SharedCleared => {
            "Les valeurs partagées sont effacées. Les instances gardent leur propre options.txt."
        }
        M::NoInstance => "Aucune instance principale",
        M::EditHereOnly => "modifier ici uniquement",
        M::Value => "Valeur",
        M::SyncFrom => "Instance principale",
        M::SyncFromHint => {
            "Les réglages modifiés en jeu dans votre instance principale sont copiés dans vos autres instances à leur démarrage."
        }
        M::SyncFromNoneHint => {
            "Sans instance principale, les réglages partagés ne changent qu’ici."
        }
        M::MainOfferTitle => "Faire de {name} votre instance principale\u{202F}?",
        M::MainOfferBody => {
            "Les réglages de jeu partagés gardent vos contrôles, la vidéo, le son et d’autres options identiques dans toutes vos instances. Les réglages modifiés en jeu dans votre instance principale sont copiés dans les autres à leur démarrage."
        }
        M::MainChangeLater => {
            "Vous pourrez changer cela dans Paramètres › Réglages de jeu partagés."
        }
        M::MakeItMain => "En faire l’instance principale",
        M::MainSwitchBody => {
            "{current} est votre instance principale. Ses changements en jeu ne mettront plus à jour vos réglages partagés\u{a0}: ce seront ceux de {name}."
        }
        M::MakeNameMain => "Rendre {name} principale",
        M::FirstTimeSetupSection => "Première configuration",
        M::ResetFirstTimeSetup => "Réinitialiser la première configuration",
        M::ResetFirstTimeSetupHint => {
            "La prochaine nouvelle instance sera de nouveau proposée comme instance principale. L’instance principale actuelle est retirée\u{a0}; les valeurs partagées restent."
        }
        M::FirstTimeSetupReset => {
            "La première configuration est réinitialisée. Créez une instance pour la voir."
        }
        M::StartFromInstance => "Partir d’une instance",
        M::CopyFromInstance => "Copier depuis une instance…",
        M::ClearShared => "Effacer les valeurs partagées",
        M::ClearSharedHint => "Les instances gardent leur propre options.txt.",
        M::ClearEveryShared => "Effacer toutes les valeurs partagées",
        M::ClearEllipsis => "Effacer…",
        M::NoValuesYet => "Aucune valeur pour l’instant",
        M::Hide => "Masquer",
        M::Show => "Afficher",
        M::ServerList => "Liste des serveurs",
        M::ServerListHint => "Bientôt disponible.",
        M::Source => "Source",
        M::WhatsShared => "Ce qui est partagé",
        M::WhatsSharedLead => "Les catégories désactivées restent propres à chaque instance.",
        M::CategoryControls => "Commandes",
        M::CategoryVideo => "Vidéo",
        M::CategorySound => "Son",
        M::CategoryAccessibility => "Accessibilité et chat",
        M::CategoryLanguage => "Langue",
        M::CategoryControlsHint => "Souris, touches",
        M::CategoryVideoHint => {
            "Champ de vision, distance d’affichage, taille de l’interface, graphismes"
        }
        M::CategorySoundHint => "Volume, sous-titres",
        M::CategoryAccessibilityHint => "Narrateur, chat, fond du texte",
        M::CategoryLanguageHint => "Langue et police du jeu",
        M::NarratorSection => "Narrateur",
        M::KeepNarratorOff => "Laisser le narrateur désactivé",
        M::KeepNarratorOffHint => "Ignore aussi l’écran d’accessibilité du premier lancement.",
        M::NarratorNotShared => {
            "« {category} » n’est pas partagé\u{a0}: chaque instance garde son propre réglage du narrateur."
        }
        M::NarratorKeptOff => {
            "Le narrateur reste désactivé dans toutes les instances, y compris les nouvelles."
        }
        M::NarratorPerInstance => {
            "Chaque instance garde à nouveau son propre réglage du narrateur."
        }

        M::SigningIn => "Connexion…",
        M::OfflineCouldntConnect => "Hors ligne – connexion impossible",
        M::AddMinecraftAccount => "Ajouter un compte Minecraft",
        M::Offline => "Hors ligne",
        M::AddMicrosoftAccount => "Ajouter un compte Microsoft",
        M::AddOfflineAccount => "Ajouter un compte hors ligne",
        M::RemoveNamed => "Retirer {name}",
        M::OfflineNote => "Hors ligne – solo et serveurs en mode hors ligne uniquement",

        M::OfflineDialogBody => {
            "Pour le développement et les tests. Le compte n’a pas de connexion Microsoft, et son UUID est celui que les serveurs en mode hors ligne donnent à ce nom."
        }
        M::Username => "Nom d’utilisateur",
        M::UsernameRule => "De 3 à 16 lettres, chiffres ou tirets bas.",
        M::AddAccount => "Ajouter le compte",

        M::LauncherIdentity => "Identité du lanceur",
        M::SignInAsLauncher => "Se connecter en tant que Minecraft Launcher",
        M::SignInAsLauncherHint => {
            "En attendant que l’application de Lodestone soit approuvée. Pour les tests uniquement."
        }
        M::SignInAsLauncherTitle => "Se connecter en tant que Minecraft Launcher\u{202F}?",
        M::WhatThisDoes => "Ce que fait cette option",
        M::WhatThisDoesBody => {
            "Lodestone Launcher indiquera à Microsoft qu’il est le Minecraft Launcher officiel de Mojang, en utilisant l’identifiant client public de ce lanceur, au lieu de l’application enregistrée de Lodestone. Microsoft et Mojang ne l’ont pas approuvé, Lodestone n’est pas affilié à ces sociétés, et cela peut enfreindre les conditions de Microsoft pour ses applications."
        }
        M::Risks => "Risques",
        M::RisksBody => {
            "Microsoft peut bloquer cela à tout moment. Dans ce cas, les comptes connectés de cette façon cessent de fonctionner. Utilisez-la uniquement pour des tests personnels, jusqu’à ce que l’application de Lodestone soit approuvée."
        }
        M::WhyHere => "Pourquoi cette option",
        M::WhyHereBody => {
            "L’application de Lodestone attend encore l’approbation de Mojang, la connexion Microsoft normale ne peut donc pas encore aboutir."
        }
        M::WhenSignIn => "Lors de la connexion",
        M::WhenSignInBody => {
            "«\u{a0}{add}\u{a0}» vous donne un code. Saisissez-le sur microsoft.com/link et connectez-vous à votre compte Microsoft sur cette page. Les pages de Microsoft peuvent afficher le nom de Minecraft au lieu de celui de Lodestone. Si le code ne fonctionne pas, vous pouvez vous connecter avec un lien\u{a0}: votre navigateur ouvre login.live.com, et vous recollez l’adresse de la page vide sur laquelle il s’arrête."
        }
        M::WhatLauncherGets => "Ce que le lanceur obtient",
        M::WhatLauncherGetsBody => {
            "La même chose qu’avec une connexion normale\u{a0}: des jetons qui lui permettent de jouer à Minecraft avec ce compte. Celui de longue durée est conservé dans votre {keychain}, les autres seulement en mémoire. Ces comptes sont marqués «\u{a0}{badge}\u{a0}» dans le menu du compte."
        }
        M::HowToUndo => "Comment l’annuler",
        M::HowToUndoBody => {
            "Désactivez ce réglage et déconnectez ces comptes. Lodestone Launcher propose de les déconnecter quand vous le désactivez."
        }
        M::TurnItOn => "J’ai compris, l’activer",
        M::TurnOffLauncherTitle => {
            "Désactiver la connexion en tant que Minecraft Launcher\u{202F}?"
        }
        M::TurnOffLauncherBody => {
            "Les nouvelles connexions Microsoft utiliseront de nouveau l’application de Lodestone."
        }
        M::OneLauncherAccount => "{name} est connecté en tant que Minecraft Launcher.",
        M::TurnOffLauncherAccounts => {
            "{accounts} Les comptes gardés continuent de fonctionner jusqu’à ce que Microsoft bloque cela. Les déconnecter les retire de Lodestone Launcher et supprime leurs connexions enregistrées de votre {keychain}."
        }
        M::TurnOffAndSignOut => "Désactiver et déconnecter",
        M::TurnOffOnly => "Désactiver seulement",
        M::SignInSection => "Connexion Microsoft",
        M::CommandLineSection => "Ligne de commande",
        M::CommandLineLead => {
            "lodestone-launcher-cli et les autres programmes que vous lancez joignent le lanceur par un socket dans son dossier de données. Seul votre compte utilisateur peut s’y connecter. Ces réglages ne se changent qu’ici, jamais depuis la ligne de commande."
        }
        M::AllowCli => "Autoriser les connexions en ligne de commande",
        M::AllowCliHint => {
            "Désactivé, le socket est fermé et les programmes connectés sont déconnectés."
        }
        M::CliAccess => "Accès",
        M::CliAccessHint => {
            "Lecture seule répond aux questions mais refuse tout ce qui modifie les instances, les réglages, les comptes ou les fichiers."
        }
        M::CliFullControl => "Contrôle total",
        M::CliReadOnly => "Lecture seule",
        M::DebugTraces => "Traces de débogage",
        M::DebugTracesHint => {
            "Enregistre ce sur quoi vous cliquez et ce que fait le lanceur, pour lodestone-launcher-cli trace. Désactivé, rien n’est enregistré."
        }
        M::CliGameLogs => "Journaux du jeu",
        M::CliGameLogsHint => "Les programmes peuvent lire la sortie de la console du jeu.",
        M::SocketPath => "Socket",
        M::CopyPath => "Copier le chemin",
        M::OpenDataFolder => "Ouvrir le dossier de données",
        M::SocketClosed => "Fermé",
        M::SocketError => "Le socket n’a pas pu s’ouvrir : {error}",
        M::ConnectedClients => "Programmes connectés",
        M::NoClients => "Aucun programme n’est connecté.",
        M::UnnamedClient => "Programme sans nom",
        M::ClientSince => "Connecté · {time}",
        M::ClientPid => "Processus {pid}",
        M::Disconnect => "Déconnecter",
        M::DiagnosticsSection => "Diagnostic",
        M::PerfMonitor => "Moniteur de performances",
        M::PerfMonitorHint => {
            "Une petite fenêtre avec la mémoire, le processeur et les temps d’image du lanceur, et ceux d’un jeu en cours, mis à jour chaque seconde."
        }
        M::PerfOpen => "Ouvrir le moniteur",
        M::PerfLauncher => "Lanceur",
        M::PerfMemory => "Mémoire",
        M::PerfCpu => "Processeur",
        M::PerfCpuValue => "{percent} d’un cœur · {cores} cœurs",
        M::PerfThreads => "Threads",
        M::PerfOpenFiles => "Fichiers ouverts",
        M::PerfRendering => "Affichage",
        M::PerfFps => "Images par seconde",
        M::PerfFrameTime => "Temps par image",
        M::PerfFrameStats => "moy. {avg} · p95 {p95} · max {max}",
        M::PerfIdle => "Au repos : rien n’a été redessiné depuis 2 secondes",
        M::PerfGame => "Jeu",
        M::PerfNoGame => "Aucun jeu n’est lancé.",
        M::PerfInternals => "Dans le lanceur",
        M::PerfImages => "Images décodées",
        M::PerfContentCache => "Cache du contenu",
        M::PerfJobs => "Tâches",
        M::PerfDownloads => "Téléchargements",
        M::PerfCliClients => "Clients CLI",
        M::PerfTraceBus => "Bus de traces",
        M::PerfTraceValue => "{subscribers} à l’écoute · {rate} évén./s",
        M::PerfRate => "{rate}/s",
        M::PerfSelfNote => {
            "Mesuré chaque seconde tant que cette fenêtre est ouverte. Elle se mesure aussi elle-même : ses propres rafraîchissements ajoutent un peu de processeur."
        }
        M::PerfCopy => "Copier l’instantané",
        M::PerfCopied => "Copié",
        M::PerfLastMinute => "60 dernières s",
        M::PerfUnknown => "–",
        M::PerfOff => "Désactivé",

        M::SignInWithMicrosoft => "Se connecter avec Microsoft",
        M::LauncherIdentityNote => {
            "Option de développement\u{a0}: connexion en tant que Minecraft Launcher, et non en tant que Lodestone Launcher. Les pages de Microsoft peuvent afficher le nom de Minecraft au lieu de celui de Lodestone."
        }
        M::UseLinkInstead => "Utiliser plutôt un lien de connexion",
        M::UseCodeInstead => "Utiliser plutôt un code",
        M::SignInCodeBody => {
            "Connectez-vous avec le compte Microsoft avec lequel vous jouez à Minecraft: Java Edition. Vous recevrez un code à saisir sur le site de Microsoft."
        }
        M::SignInBrowserBody => {
            "Connectez-vous avec le compte Microsoft avec lequel vous jouez à Minecraft: Java Edition. Votre navigateur s’ouvre pour terminer la connexion."
        }
        M::KeepsSignIn => "Lodestone Launcher conserve votre connexion dans le {keychain}.",
        M::GetCode => "Obtenir un code",
        M::WaitingForBrowser => "En attente de votre navigateur…",
        M::FinishInBrowser => {
            "Terminez la connexion dans la fenêtre du navigateur qui s’est ouverte, puis revenez ici."
        }
        M::OpenBrowserAgain => "Rouvrir le navigateur",
        M::GettingCode => "Obtention d’un code…",
        M::GoToHost => {
            "Allez sur {host} depuis n’importe quel appareil et saisissez ce code\u{a0}:"
        }
        M::CodeCopiedWaiting => "Code copié. En attente de sa saisie…",
        M::WaitingForCode => "En attente de la saisie du code…",
        M::CopyCodeAndOpen => "Copier le code et ouvrir {host}",
        M::CopySignInCode => "Copier le code",
        M::StopSigningInTitle => "Arrêter la connexion\u{202F}?",
        M::StopSigningInBody => {
            "Microsoft attend toujours le code que vous avez copié. Si vous arrêtez maintenant, le saisir ne vous connectera pas."
        }
        M::StopSigningIn => "Arrêter la connexion",
        M::KeepWaiting => "Continuer d’attendre",
        M::LinkInstructions => {
            "Connectez-vous dans la fenêtre du navigateur qui s’est ouverte. Microsoft affiche ensuite une page indiquant qu’elle n’est normalement pas affichée. Copiez l’adresse complète depuis la barre d’adresse du navigateur et collez-la uniquement dans ce lanceur."
        }
        M::LinkStartsWith => "Elle commence par {url}",
        M::SigningInToMinecraft => "Connexion à Minecraft…",
        M::ExpectedUntilApproved => {
            "C’est normal tant que Mojang n’a pas approuvé l’application. Votre compte n’a aucun problème."
        }

        M::AuthCancelled => "Connexion annulée",
        M::AuthCancelledBody => "La connexion a été annulée.",
        M::AuthTimedOut => "Délai de connexion dépassé",
        M::AuthTimedOutBody => {
            "La connexion dans le navigateur n’a pas été terminée à temps. Réessayez, ou utilisez plutôt un code."
        }
        M::AuthStateMismatch => "La connexion ne correspond pas",
        M::AuthStateMismatchBody => {
            "Le navigateur est revenu avec une autre demande de connexion, que Lodestone Launcher a donc ignorée. Réessayez."
        }
        M::AuthDeclined => "Connexion refusée",
        M::AuthDeclinedBody => {
            "La connexion a été refusée dans le navigateur. Réessayez et acceptez les autorisations pour continuer."
        }
        M::AuthMicrosoft => "Échec de la connexion Microsoft",
        M::AuthMicrosoftBody => {
            "Microsoft n’a pas pu vous connecter ({code}). Réessayez dans un instant."
        }
        M::AuthMicrosoftSaid => "Microsoft a répondu\u{a0}: {description} ({code})",
        M::AuthCodeExpired => "Le code a expiré",
        M::AuthCodeExpiredBody => {
            "Le code a expiré avant la fin de la connexion. Obtenez un nouveau code et réessayez."
        }
        M::AuthSignInExpiredBody => {
            "Votre connexion Microsoft enregistrée a expiré ou a été révoquée. Reconnectez-vous pour continuer à utiliser ce compte."
        }
        M::AuthNoXbox => "Aucun profil Xbox",
        M::AuthNoXboxBody => {
            "Ce compte Microsoft n’a pas encore de profil Xbox. Connectez-vous une fois sur xbox.com pour en créer un, puis réessayez."
        }
        M::AuthRegion => "Xbox Live n’est pas disponible",
        M::AuthRegionBody => {
            "Xbox Live n’est pas disponible dans le pays ou la région de ce compte, qui ne peut donc pas se connecter à Minecraft."
        }
        M::AuthAdult => "Vérification d’âge requise",
        M::AuthAdultBody => {
            "Ce compte doit passer la vérification d’âge adulte sur le site Xbox avant de pouvoir jouer. C’est obligatoire en Corée du Sud."
        }
        M::AuthChild => "Compte enfant",
        M::AuthChildBody => {
            "Ceci est un compte enfant. Un adulte doit l’ajouter à un groupe familial Microsoft avant qu’il puisse se connecter."
        }
        M::AuthXbox => "Échec de la connexion Xbox Live",
        M::AuthXboxBody => "Xbox Live a refusé la connexion (erreur {code}). Réessayez plus tard.",
        M::AuthNotApproved => "Pas encore approuvé par Minecraft",
        M::AuthNotApprovedBody => {
            "La connexion Microsoft fonctionne, mais Minecraft n’a pas encore approuvé Lodestone Launcher. Utilisez le mode hors ligne pour l’instant."
        }
        M::AuthNotOwned => "Minecraft introuvable",
        M::AuthNotOwnedBody => {
            "Ce compte Microsoft ne possède pas Minecraft: Java Edition. Si vous jouez via le Xbox Game Pass, lancez Minecraft une fois dans le lanceur officiel, puis réessayez."
        }
        M::AuthNoProfile => "Aucun profil Minecraft",
        M::AuthNoProfileBody => {
            "Ce compte n’a pas encore de profil Minecraft. Si vous jouez via le Xbox Game Pass, lancez Minecraft une fois dans le lanceur officiel pour choisir un nom, puis réessayez."
        }
        M::AuthRateLimited => "Trop de tentatives",
        M::AuthRateLimitedBody => {
            "Trop de tentatives de connexion. Patientez quelques minutes et réessayez."
        }
        M::AuthNetwork => "Connexion impossible",
        M::AuthNetworkBody => {
            "Lodestone Launcher n’a pas pu joindre les serveurs de connexion. Vérifiez votre connexion Internet et réessayez."
        }
        M::AuthUnexpected => "Échec de la connexion",
        M::AuthUnexpectedBody => {
            "Un problème est survenu pendant l’étape «\u{a0}{step}\u{a0}» (HTTP {status}). Réessayez dans un instant."
        }
        M::AuthListener => "Impossible de démarrer la connexion",
        M::AuthListenerBody => {
            "Lodestone Launcher n’a pas pu ouvrir la page locale vers laquelle le navigateur revient. Utilisez plutôt un code."
        }
        M::AuthKeychain => "Impossible d’enregistrer la connexion",
        M::AuthKeychainBody => {
            "Lodestone Launcher n’a pas pu enregistrer la connexion dans votre {keychain}. Réessayez."
        }
        M::AuthInvalidLink => "Ce n’est pas la bonne adresse",
        M::AuthNotAUrlBody => {
            "Ce n’est pas une adresse web. Copiez l’adresse complète depuis la barre d’adresse de votre navigateur après vous être connecté."
        }
        M::AuthWrongPageBody => {
            "Seule une adresse commençant par {url} est acceptée. Copiez-la depuis la barre d’adresse de la page vide que Microsoft affiche une fois connecté."
        }
        M::AuthNoCodeBody => {
            "Cette adresse ne contient aucun code de connexion. Terminez d’abord la connexion, puis copiez l’adresse de la page sur laquelle vous arrivez."
        }
        M::StepMicrosoftToken => "connexion Microsoft",
        M::StepDeviceCode => "connexion Microsoft par code",
        M::StepXboxLive => "connexion Xbox Live",
        M::StepXsts => "autorisation Xbox Live",
        M::StepMinecraftLogin => "connexion Minecraft",
        M::StepEntitlements => "vérification de la possession de Minecraft",
        M::StepProfile => "chargement du profil Minecraft",

        M::AccountsTooNew => {
            "Vos comptes ont été enregistrés par une version plus récente de Lodestone Launcher. Les modifications faites ici ne seront pas enregistrées."
        }
        M::AccountsCorrupt => {
            "Le fichier accounts.json n’a pas pu être lu, Lodestone Launcher a donc démarré sans comptes."
        }
        M::AccountsCorruptMoved => {
            "Le fichier accounts.json n’a pas pu être lu, Lodestone Launcher a donc démarré sans comptes. L’ancien fichier se trouve dans {path}."
        }
        M::CouldntReadAccounts => "Impossible de lire vos comptes\u{a0}: {error}",
        M::CouldntSaveSettings => "Impossible d’enregistrer vos paramètres\u{a0}: {error}",
        M::CouldntSaveAccounts => "Impossible d’enregistrer vos comptes\u{a0}: {error}",
        M::CouldntRemoveSignIn => {
            "Impossible de supprimer la connexion enregistrée de {name}\u{a0}: {error}"
        }
        M::AddAccountFirst => {
            "Ajoutez d’abord un compte\u{a0}: connectez-vous avec Microsoft, ou ajoutez un compte hors ligne."
        }
        M::SignInExpired => "Reconnectez-vous à {name}\u{a0}: sa connexion enregistrée a expiré.",
        M::LauncherClosing => "Le lanceur est en cours de fermeture.",

        M::UpdatesSection => "Mises à jour",
        M::UpdateAutomatically => "Mettre à jour automatiquement",
        M::UpdateAutomaticallyHint => {
            "Télécharge les nouveaux builds depuis GitHub en arrière-plan."
        }
        M::BetaBuilds => "Builds bêta",
        M::BetaBuildsHint => {
            "Profitez des nouveautés avant tout le monde. Les builds bêta peuvent comporter des bugs. Si vous désactivez cette option, vous gardez votre build jusqu’à ce qu’un build stable plus récent sorte."
        }
        M::BuildNumber => "Build {build}",
        M::BuildNumberCommit => "Build {build} · {commit}",
        M::DevelopmentBuild => "Build de développement",
        M::UpdatesUnavailable => {
            "Ce build n’a pas été publié sur GitHub, il ne se met donc pas à jour tout seul."
        }
        M::UpdateNotChecked => {
            "Recherche de nouveaux builds sur GitHub toutes les quelques heures."
        }
        M::UpdateChecking => "Recherche d’un build plus récent…",
        M::UpToDate => "C’est le build le plus récent.",
        M::UpdateAvailable => "Le build {build} est disponible.",
        M::UpdateDownloading => "Téléchargement du build {build}… {percent}",
        M::UpdateReady => "Le build {build} est installé. Redémarrez pour l’utiliser.",
        M::UpdateReadyPlaying => {
            "Le build {build} est installé. Vous pourrez redémarrer une fois le jeu fermé."
        }
        M::UpdateFailed => "Impossible de mettre à jour\u{a0}: {error}",
        M::CheckForUpdates => "Vérifier",
        M::DownloadUpdate => "Télécharger",
        M::RestartToUpdate => "Redémarrer",
        M::UpdateReadyBadge => "Mise à jour prête",
        M::UpdateAvailableBadge => "Mise à jour disponible",
        M::MinimizeWindow => "Réduire",
        M::MaximizeWindow => "Agrandir",
        M::RestoreWindow => "Restaurer",

        M::RestOnModrinth => "La suite de la description est sur Modrinth.",
        M::Storage => "Stockage",
        M::Skin => "Thème",
        M::SkinGallery => "Thèmes",
        M::SkinImport => "Importer…",
        M::SkinImportZip => "Importer un thème en zip",
        M::SkinImportFolder => "Importer un thème en dossier",
        M::SkinImportButton => "Importer",
        M::SkinExport => "Exporter…",
        M::SkinNew => "Nouveau thème…",
        M::SkinDelete => "Supprimer",
        M::SkinDeleteConfirm => "Supprimer {name}",
        M::SkinCopyName => "Copie de {name}",
        M::SkinCopied => "Copie de {name} créée pour la modifier.",
        M::SkinImporting => "Importation…",
        M::SkinExporting => "Exportation…",
        M::SkinImported => "{name} importé.",
        M::SkinExported => "Exporté dans {path}.",
        M::SkinSafeMode => "Démarré sans thème (--no-skin).",
        M::SkinCouldntLoad => "Impossible de charger le thème\u{a0}: {error}",
        M::SkinSidebar => "Barre latérale",
        M::SkinImage => "Image",
        M::SkinChoose => "Choisir…",
        M::SkinUseImage => "Utiliser l’image",
        M::SkinRemove => "Retirer",
        M::SkinFit => "Ajustement",
        M::SkinFitCover => "Remplir",
        M::SkinFitTile => "Mosaïque",
        M::SkinTint => "Teinte",
        M::SkinTintStrength => "Intensité de la teinte",
        M::SkinBlur => "Flou",
        M::SkinMaterial => "Matière",
        M::SkinMaterialHint => {
            "Laisse voir ce qui se trouve derrière le panneau : le fond de la fenêtre, la page ou une boîte de dialogue. Flou et Givré le montrent flouté."
        }
        M::SkinMaterialTranslucent => "Translucide",
        M::SkinMaterialBlur => "Flou",
        M::SkinMaterialFrost => "Givré",
        M::SkinMaterialOpacity => "Remplissage visible",
        M::SkinMaterialBlurAmount => "Flou de l’arrière-plan",
        M::SkinMaterialFrostVeil => "Voile givré",
        M::SkinMaterialGrain => "Grain",
        M::SkinBanner => "Bannière d’instance",
        M::SkinBannerInSidebar => "Afficher derrière les lignes de la barre latérale",
        M::SkinHero => "Bannière Jouer",
        M::SkinHeroHint => "Pour les instances qui n’en ont pas.",
        M::SkinPlayButton => "Bouton Jouer",
        M::SkinColor => "Couleur",
        M::SkinFadeTo => "Dégradé vers",
        M::SkinShape => "Forme",
        M::SkinShapeRounded => "Arrondi",
        M::SkinShapePill => "Pilule",
        M::SkinShapePixel => "Pixel",
        M::SkinLabel => "Texte",
        M::SkinGlow => "Halo",
        M::SkinColors => "Couleurs",
        M::SkinAccent => "Accent",
        M::SkinProgress => "Barre de progression",
        M::SkinDefault => "Par défaut",
        M::SkinNone => "Aucune",
        M::SkinAdjusted => "L’accent a été ajusté pour rester lisible.",
        M::SkinReset => "Revenir au thème par défaut",
        M::SkinPlayColor => "Couleur du bouton Jouer",
        M::SkinUseSkins => "Du thème",
        M::SkinSurfaceWindow => "Fenêtre",
        M::SkinSurfaceTopBar => "Barre du haut",
        M::SkinSurfaceSidebar => "Barre latérale",
        M::SkinSurfaceMain => "Page",
        M::SkinSurfaceHero => "Image de Jouer",
        M::SkinSurfacePlayBar => "Barre Jouer",
        M::SkinSurfaceCard => "Cartes",
        M::SkinSurfaceSettingCard => "Groupes de réglages",
        M::SkinSurfaceRow => "Lignes de liste",
        M::SkinSurfaceDialog => "Boîtes de dialogue",
        M::SkinSurfaceSettingsNav => "Pages des réglages",
        M::SkinSurfacePopover => "Menus",
        M::SkinSurfaceInput => "Champs de texte",
        M::SkinSurfaceButtonPrimary => "Boutons principaux",
        M::SkinSurfaceButtonSecondary => "Boutons secondaires",
        M::SkinSurfaceButtonGhost => "Boutons discrets",
        M::SkinSurfaceButtonDanger => "Boutons de danger",
        M::SkinSurfaceToggle => "Interrupteurs",
        M::SkinSurfaceTab => "Onglets",
        M::SkinSurfacePill => "Pastilles",
        M::SkinSurfacePlayButton => "Bouton Jouer",
        M::SkinSurfaceProgress => "Barres de progression",
        M::SkinSurfaceToast => "Notifications",
        M::SkinRoleText => "texte",
        M::SkinRoleMuted => "texte discret",
        M::SkinRoleFaint => "indices et icônes",
        M::SkinRoleLabel => "libellé",
        M::SkinCheckShort => "{surface}, {role}\u{a0}: {ratio}:1, il faut {target}:1",
        M::SkinCheckNoFix => {
            "Aucune couleur de texte n’y arrive sur ce fond\u{a0}; changez le fond."
        }
        M::SkinApplyFix => "Appliquer la correction suggérée",
        M::SkinFixAll => "Tout corriger",
        M::SkinChecksPass => "Tout le texte est lisible ({count} vérifications).",
        M::SkinChecksNote => "La lisibilité dépend du thème\u{a0}: ce sont des suggestions.",
        M::SkinBanners => "Du thème",
        M::SkinBannerMissing => {
            "Le thème de cette bannière n’existe plus\u{a0}; celle par défaut s’affiche."
        }
        M::SkinEditorTitle => "Éditeur de thème",
        M::SkinPopOut => "Ouvrir l’éditeur",
        M::SkinEditorLead => {
            "Modifiez chaque surface, police et bannière dans l’éditeur. Les changements s’affichent dans la fenêtre au fur et à mesure, et Annuler les reprend."
        }
        M::SkinSummary => "{surfaces} surfaces peintes · {fonts} polices · {banners} bannières",
        M::SkinSurfaces => "Surfaces",
        M::SkinTokens => "Couleurs du texte",
        M::SkinFonts => "Polices",
        M::SkinBannerSet => "Bannières",
        M::SkinPlayButtonSection => "Bouton Jouer",
        M::SkinReadability => "Lisibilité",
        M::SkinFill => "Remplissage",
        M::SkinFillColour => "Couleur",
        M::SkinFillGradient => "Dégradé",
        M::SkinOpacity => "Opacité",
        M::SkinGradientLinear => "Linéaire",
        M::SkinGradientRadial => "Radial",
        M::SkinAngle => "Angle",
        M::SkinStops => "Couleurs",
        M::SkinStopAt => "Position",
        M::SkinAddStop => "Ajouter une couleur",
        M::SkinRemoveStop => "Retirer la couleur",
        M::SkinBuiltinImages => "Images intégrées",
        M::SkinBorder => "Bordure",
        M::SkinBorderWidth => "Épaisseur de bordure",
        M::SkinRadius => "Coins",
        M::SkinTextColour => "Texte",
        M::SkinMutedColour => "Texte discret",
        M::SkinTextElsewhere => "Le texte de cette surface suit les couleurs du texte.",
        M::SkinTokenText => "Texte",
        M::SkinTokenSecondary => "Libellés",
        M::SkinTokenMuted => "Descriptions",
        M::SkinTokenFaint => "Indices",
        M::SkinTokenBorder => "Lignes",
        M::SkinFontHeading => "Titres",
        M::SkinFontBody => "Texte courant",
        M::SkinFontSidebar => "Barre latérale",
        M::SkinFontButton => "Boutons",
        M::SkinFontPlay => "Bouton Jouer et titre",
        M::SkinFontTab => "Onglets",
        M::SkinFontMono => "Code et journaux",
        M::SkinFontAdd => "Ajouter un fichier de police…",
        M::SkinBannerAdd => "Ajouter une bannière…",
        M::SkinBannersNone => {
            "Pas encore de bannière. Les nouvelles instances gardent l’image Jouer par défaut."
        }
        M::SkinBannersLead => {
            "Une nouvelle instance en prend une au hasard\u{a0}; chacune peut en changer dans son arrière-plan Jouer."
        }
        M::SkinSaveAsNew => "Enregistrer comme nouveau thème",
        M::SkinUseLaunchers => "Utiliser celui du lanceur",
        M::SkinPreviewIn => "Montrer dans la fenêtre",
        M::SkinPreviewSettings => "Réglages",
        M::SkinPreviewMenu => "Un menu",
        M::SkinPreviewNotice => "Une notification",
        M::SkinPreviewNoticeText => "Voici l’apparence des notifications.",
        M::SkinSampleText => "Texte d’exemple",
        M::SkinSampleMuted => "Texte plus discret",
        M::SkinScrim => "Ombre sous le titre",
        M::StorageInstances => "Instances",
        M::StorageGameFiles => "Fichiers du jeu",
        M::StorageRuntimes => "Environnements Java",
        M::StorageCache => "Cache",
        M::StorageSnapshots => "Instantanés",
        M::StorageSnapshotsHint => "{count} instantanés · {size} récupérables",
        M::StorageCleanSnapshotsTitle => "Supprimer les instantanés automatiques\u{a0}?",
        M::StorageCleanSnapshotsBody => {
            "Libère {size}. Supprime les instantanés pris avant un retour en arrière, ceux des instances supprimées et les fichiers créés pour le partage."
        }
        M::StorageLeftovers => "Téléchargements et importations inachevés",
        M::StorageCleanLeftoversTitle => "Supprimer les téléchargements inachevés\u{a0}?",
        M::StorageCleanLeftoversBody => {
            "Cela libère {size}. Ce sont les fichiers de téléchargements et d’importations de modpacks qui n’ont pas abouti."
        }
        M::StorageOther => "Réglages et autres fichiers",
        M::StorageCacheAndLogs => "Cache et journaux",
        M::StorageLogs => "Journaux et rapports de plantage",
        M::StorageOldLogs => "{size} de plus de 30\u{a0}jours",
        M::StorageVersions => "Versions",
        M::StorageLibraries => "Bibliothèques",
        M::StorageAssets => "Ressources",
        M::StorageUnused => "Utilisé par aucune instance",
        M::StorageUnusedTag => "Inutilisé",
        M::StorageJava => "Java {major}",
        M::StorageWorlds => "Mondes {size}",
        M::StorageContent => "Mods et packs {size}",
        M::StorageOtherFiles => "Autres {size}",
        M::StorageNoInstances => "Aucune instance",
        M::StorageNoRuntimes => "Aucun environnement Java",
        M::StorageCounting => "Calcul…",
        M::StorageCouldntCount => "Impossible de compter les fichiers\u{a0}: {error}",
        M::StorageStopGame => {
            "Arrêtez le jeu pour supprimer des fichiers du jeu, des environnements Java ou le cache."
        }
        M::StorageOpenFolder => "Ouvrir le dossier",
        M::StorageOpen => "Ouvrir",
        M::StorageDelete => "Supprimer…",
        M::StorageRemoveEllipsis => "Supprimer…",
        M::StorageClearEllipsis => "Vider…",
        M::StorageRemove => "Supprimer",
        M::StorageClear => "Vider",
        M::StorageDeleteOld => "Supprimer les anciens…",
        M::StorageDeleteAll => "Tout supprimer…",
        M::StorageDeleteConfirm => "Supprimer",
        M::StorageFreed => "{size} libérés",
        M::StorageCouldntRemove => "Impossible de tout supprimer\u{a0}: {error}",
        M::StorageCleanGameFilesTitle => "Supprimer les fichiers du jeu inutilisés\u{a0}?",
        M::StorageCleanGameFilesBody => {
            "Cela libère {size}. Les versions, bibliothèques et ressources qu’aucune instance n’utilise sont supprimées, et se retéléchargent si une instance en a besoin."
        }
        M::StorageCleanRuntimesTitle => "Supprimer les environnements Java inutilisés\u{a0}?",
        M::StorageCleanRuntimesBody => {
            "Cela libère {size}. Ils se retéléchargent quand une version de Minecraft en a besoin."
        }
        M::StorageClearCacheTitle => "Vider le cache\u{a0}?",
        M::StorageClearCacheBody => {
            "Cela libère {size}. Le launcher retélécharge ce dont il a besoin."
        }
        M::StorageDeleteOldLogsTitle => "Supprimer les anciens journaux\u{a0}?",
        M::StorageDeleteOldLogsBody => {
            "Cela libère {size}. Les journaux et rapports de plantage de plus de 30\u{a0}jours sont supprimés définitivement."
        }
        M::StorageDeleteAllLogsTitle => "Supprimer tous les journaux\u{a0}?",
        M::StorageDeleteAllLogsBody => {
            "Cela libère {size}. Tous les journaux et rapports de plantage sont supprimés définitivement, sauf ceux d’un jeu en cours."
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AndMore => ["et {count} autre", "et {count} autres"],
        P::Values => ["{count} valeur", "{count} valeurs"],
        P::CopiedValues => [
            "{count} valeur copiée depuis {name}.",
            "{count} valeurs copiées depuis {name}.",
        ],
        P::LauncherAccounts => [
            "{count} compte est connecté en tant que Minecraft Launcher\u{a0}: {names}.",
            "{count} comptes sont connectés en tant que Minecraft Launcher\u{a0}: {names}.",
        ],
        P::Subscriptions => ["{count} abonnement", "{count} abonnements"],
        P::PerfEntries => ["{count} entrée", "{count} entrées"],
    }
}
