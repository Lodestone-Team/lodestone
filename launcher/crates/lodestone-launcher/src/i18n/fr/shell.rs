use crate::i18n::{ShellMsg as M, ShellPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::QuitApp => "Quitter {app}",
        M::Language => "Langue",
        M::LanguageHint => {
            "La langue d’affichage de Lodestone Launcher. Système utilise la première de vos langues préférées qui est disponible, sinon l’anglais."
        }
        M::LanguageSystem => "Système",
        M::LanguageSystemIs => "Système ({language})",

        M::Play => "Jouer",
        M::Playing => "En jeu",
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
        M::DefaultsLead => {
            "Toutes les instances les utilisent, sauf si leur propre onglet Paramètres les modifie."
        }
        M::SharedLead => {
            "Un seul ensemble de réglages options.txt pour toutes les instances\u{a0}: souris, touches, vidéo, son et plus encore. Une instance peut remplacer des valeurs une à une dans son onglet Paramètres."
        }
        M::GeneralLead => "La langue du lanceur et la façon dont il liste vos instances.",
        M::AccountsLead => "Le bouton Jouer se connecte avec le compte utilisé.",
        M::DeveloperLead => "Options pour tester Lodestone Launcher.",
        M::NoAccount => "Aucun compte",
        M::ManageAccounts => "Gérer les comptes",
        M::GroupAutomatically => "Grouper les instances automatiquement",
        M::GroupAutomaticallyHint => {
            "La barre latérale classe les instances sans groupe par chargeur, comme «\u{a0}{fabric}\u{a0}» ou «\u{a0}{vanilla}\u{a0}», et range celles importées d’autres lanceurs dans «\u{a0}{imported}\u{a0}»."
        }
        M::OfflineAccount => "Compte hors ligne",
        M::MicrosoftAccount => "Compte Microsoft",
        M::SignInAgain => "Se reconnecter",
        M::InUse => "Utilisé",
        M::Use => "Utiliser",
        M::AddAnAccount => "Ajouter un compte",
        M::AddAccountHintWithOffline => {
            "Un compte Microsoft joue en ligne. Un compte hors ligne joue en solo et sur les serveurs en mode hors ligne."
        }
        M::AddAccountHint => {
            "Connectez-vous avec le compte Microsoft avec lequel vous jouez à Minecraft."
        }
        M::AddMicrosoftEllipsis => "Microsoft…",
        M::AddOfflineEllipsis => "Hors ligne…",

        M::Memory => "Mémoire",
        M::MemoryError => "Mémoire\u{a0}: {error}",
        M::MemoryDefaultHint => {
            "{total} Par défaut\u{a0}: de {min} à {max}, soit la moitié de la mémoire de {computer}, au plus {limit}."
        }
        M::MegabytesUnit => "Mo",
        M::Window => "Fenêtre",
        M::WindowHint => "Laissez la taille vide pour celle du jeu par défaut.",
        M::Fullscreen => "Plein écran",
        M::JvmArguments => "Arguments JVM",
        M::JvmArgumentsHint => {
            "Séparés par des espaces\u{202F}; mettez entre guillemets les valeurs qui contiennent des espaces."
        }
        M::GameArguments => "Arguments du jeu",
        M::GameArgumentsHint => "Transmis à Minecraft lui-même.",
        M::EnvironmentVariables => "Variables d’environnement",
        M::EnvironmentVariablesHint => "NOM=valeur, séparées par des espaces.",

        M::SyncedFrom => {
            "Les réglages modifiés en jeu dans {name} mettent désormais à jour toutes les instances."
        }
        M::SyncedFromNone => {
            "Aucune instance ne met plus à jour les paramètres partagés\u{202F}; modifiez-les ici."
        }
        M::SharedCleared => {
            "Les valeurs partagées sont effacées. Les instances gardent leur propre options.txt."
        }
        M::NoInstance => "Aucune instance",
        M::EditHereOnly => "modifier ici uniquement",
        M::Value => "Valeur",
        M::SyncFrom => "Synchroniser depuis",
        M::SyncFromHint => {
            "Les réglages que vous modifiez en jeu dans cette instance deviennent les réglages partagés. Les autres instances ne font que les recevoir."
        }
        M::SyncFromNoneHint => {
            "Sans instance, rien n’est relu depuis les jeux, et les valeurs partagées ne changent qu’ici."
        }
        M::StartFromInstance => "Partir d’une instance",
        M::StartFromInstanceHint => {
            "Copie le fichier options.txt d’une instance dans les valeurs partagées."
        }
        M::CopyFromInstance => "Copier depuis une instance…",
        M::ClearShared => "Effacer les valeurs partagées",
        M::ClearSharedHint => "Les instances gardent leur propre options.txt.",
        M::ClearEveryShared => "Effacer toutes les valeurs partagées",
        M::ClearEllipsis => "Effacer…",
        M::NoValuesYet => "Aucune valeur pour l’instant",
        M::Hide => "Masquer",
        M::Show => "Afficher",
        M::ServerList => "Liste des serveurs",
        M::ServerListHint => {
            "Bientôt disponible. Les packs de ressources et de shaders restent propres à chaque instance."
        }
        M::Source => "Source",
        M::WhatsShared => "Ce qui est partagé",
        M::WhatsSharedLead => "Une catégorie désactivée reste séparée dans chaque instance.",
        M::CategoryControls => "Commandes",
        M::CategoryVideo => "Vidéo",
        M::CategorySound => "Son",
        M::CategoryAccessibility => "Accessibilité et chat",
        M::CategoryLanguage => "Langue",
        M::CategoryControlsHint => {
            "Sensibilité de la souris, inversion, entrée brute, touches, accroupissement et sprint en bascule"
        }
        M::CategoryVideoHint => {
            "Champ de vision, distance d’affichage et de simulation, taille de l’interface, luminosité, plein écran, VSync, FPS max., particules, graphismes"
        }
        M::CategorySoundHint => "Tous les curseurs de volume, sous-titres, son directionnel",
        M::CategoryAccessibilityHint => {
            "Narrateur, fond du texte, taille et opacité du chat, effets"
        }
        M::CategoryLanguageHint => "Langue et police du jeu",
        M::NarratorSection => "Narrateur",
        M::KeepNarratorOff => "Laisser le narrateur désactivé",
        M::KeepNarratorOffHint => {
            "Dans toutes les instances, y compris les nouvelles\u{a0}: le narrateur reste désactivé, et l’écran d’accessibilité du premier lancement de Minecraft, qui se lit à voix haute, est ignoré."
        }
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
            "Les nouvelles connexions Microsoft utilisent l’application du Minecraft Launcher officiel au lieu de celle de Lodestone, en attendant que Mojang approuve celle de Lodestone. Pour des tests personnels."
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

        M::RestOnModrinth => "La suite de la description est sur Modrinth.",
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
    }
}
