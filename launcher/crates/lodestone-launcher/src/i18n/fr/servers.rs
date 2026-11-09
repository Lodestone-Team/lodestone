use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::PageHint => {
            "Les serveurs de vos Lodestone Cores. Créez une instance qui correspond à l’un d’eux, puis jouez dessus."
        }
        M::OnThisComputer => "Sur cet ordinateur",
        M::OnThisComputerHint => {
            "Lodestone Host et Lodestone Core qui tournent ici. Se connecter en tant que propriétaire prouve que ce lanceur peut accéder aux fichiers du core\u{202F}: aucun mot de passe n’est nécessaire."
        }
        M::Rescan => "Rechercher à nouveau",
        M::ScanningLocal => "Recherche de cores sur cet ordinateur…",
        M::NoLocalCores => "Aucun Lodestone Host ni Lodestone Core ne tourne sur cet ordinateur.",
        M::ConnectAsOwner => "Se connecter en tant que propriétaire",
        M::Connected => "Connecté",
        M::ChooseDataFolder => "Choisir le dossier…",
        M::ChooseDataFolderPrompt => "Choisir",
        M::ProductHostHint => "l’application de bureau, avec le tableau de bord dans une fenêtre",
        M::ProductCoreHint => "un core seul (le programme serveur, ou Docker)",
        M::ConnectedAsOwner => "Connecté à {name} en tant que propriétaire",
        M::ConnectedMadeOwner => {
            "Il n’avait pas encore de propriétaire\u{202F}: Lodestone Launcher a créé le compte propriétaire {username}. Son mot de passe est sur la carte du core\u{202F}; gardez-le en lieu sûr."
        }
        M::ConnectedOwnerKey => {
            "Ce lanceur a maintenant une clé de propriétaire pour ce core. Vous pouvez la révoquer depuis le tableau de bord du core."
        }
        M::OwnerSignIn => "Connexion du propriétaire",
        M::OwnerKey => "Clé de propriétaire",
        M::ShowPassword => "Afficher",
        M::HidePassword => "Masquer",
        M::CopyUsername => "Copier le nom d’utilisateur",
        M::CopyPassword => "Copier le mot de passe",
        M::CopyApiKey => "Copier la clé d’API",
        M::StartedBy => "Démarré par {who}",
        M::StartedDirectly => "Démarré directement",
        M::DirectNote => {
            "Ce core a été démarré directement. Lodestone Core est fait pour être lancé avec lodestone_cli, qui l’installe et le garde à jour."
        }
        M::GetLodestoneCli => "Obtenir lodestone_cli",
        M::CredentialsReadFailed => {
            "Impossible de lire les clés de propriétaire des cores de cet ordinateur"
        }
        M::CredentialsSaveFailed => "Impossible d’enregistrer la clé de propriétaire",
        M::ClaimNoCore => "Aucun Lodestone Host ni Lodestone Core ne tourne sur cet ordinateur.",
        M::ClaimRefused => {
            "Le core a refusé l’accès propriétaire. Activez «\u{a0}Laisser les applications de cet ordinateur obtenir l’accès propriétaire\u{a0}» dans ses paramètres, ou mettez-le à jour."
        }
        M::ClaimFolderNotFound => {
            "Impossible de trouver le dossier de données de ce core. Choisissez le dossier où il garde ses données (son LODESTONE_PATH). Un core dans Docker doit avoir son dossier de données monté depuis cet ordinateur."
        }
        M::ClaimNotThisCore => {
            "Le core qui a répondu n’est pas celui dont le dossier a été trouvé\u{202F}: rien n’a été enregistré."
        }
        M::ClaimNoResult => "Le core n’a pas terminé la demande. Réessayez.",
        M::ClaimNoCertificate => {
            "Ce core n’a pas de certificat HTTPS\u{202F}: Lodestone Launcher ne peut donc pas garantir que sa clé de propriétaire n’ira qu’à lui. Activez HTTPS dans les paramètres du core, puis reconnectez-vous."
        }
        M::ClaimFiles => "Impossible d’utiliser le dossier de données du core\u{a0}: {error}",
        M::ClaimInvalid => {
            "Le core a envoyé quelque chose que Lodestone Launcher ne comprend pas\u{a0}: {error}"
        }
        M::AddServer => "Ajouter un serveur",
        M::Dismiss => "Masquer",
        M::NoServersYet => "Aucun serveur pour l’instant",
        M::NoServersHint => {
            "Ajoutez un Lodestone Core par son adresse. Sans vous connecter, vous voyez les serveurs que son propriétaire liste publiquement."
        }

        M::BadgeTrusted => "De confiance",
        M::BadgePinned => "Certificat épinglé",
        M::SignedInAs => "Connecté en tant que {username}",
        M::SignedIn => "Connecté",
        M::NotSignedIn => "Non connecté\u{a0}: serveurs publics uniquement",
        M::TrustedForMods => "Confiance pour les mods",
        M::Offline => {
            "Hors ligne\u{a0}: impossible de joindre ce core. Vérifiez qu’il est en marche et sur le même réseau."
        }
        M::NotListedPublicly => {
            "Le propriétaire n’a pas activé «\u{a0}Lister les serveurs publiquement\u{a0}» dans les paramètres du core, il n’y a donc rien à afficher sans vous connecter."
        }
        M::NoPublicServers => {
            "Aucun serveur n’est listé publiquement. Connectez-vous pour voir ceux auxquels vous avez accès."
        }
        M::NoVisibleServers => "Aucun serveur visible pour vous sur ce core.",
        M::CertChangedWarning => {
            "Le certificat HTTPS de ce core a changé depuis que vous lui avez fait confiance, Lodestone Launcher refuse donc de s’y connecter. Le core crée un nouveau certificat quand le sien arrive à expiration ou que son adresse change\u{202F}; cela peut aussi vouloir dire que quelqu’un intercepte la connexion. Vérifiez la nouvelle empreinte avant de lui faire confiance."
        }
        M::CertUntrustedWarning => {
            "Le certificat HTTPS de ce core n’est pas approuvé par {computer}. Les Lodestone Cores créent leur propre certificat, c’est donc normal\u{a0}: vérifiez son empreinte, puis faites-lui confiance."
        }
        M::ReviewNewCertificate => "Approuver le nouveau certificat…",
        M::ReviewCertificate => "Approuver le certificat…",

        M::Details => "Détails",
        M::CreateInstance => "Créer l’instance",
        M::StartAndJoin => "Démarrer et rejoindre",
        M::Play => "Jouer",
        M::More => "Plus…",
        M::InstanceReady => "Instance prête",
        M::AddressHidden => "Le propriétaire n’a pas partagé l’adresse de ce serveur.",

        M::StateStarting => "Démarrage",
        M::StateRunning => "En marche",
        M::StateStopping => "Arrêt en cours",
        M::StateStopped => "Arrêté",
        M::StateCrashed => "Planté",
        M::StateUnknown => "Inconnu",
        M::StateHidden => "État masqué",
        M::LoaderOther => "Autre",
        M::PlayersOfMax => "{count}/{max} joueurs",

        M::BusyFetching => "Récupération des mods du serveur…",
        M::BusyCreating => "Création de l’instance, téléchargement…",
        M::BusyCreatingFiles => {
            "Création de l’instance, téléchargement de {done} fichiers sur {total}…"
        }
        M::BusyChecking => "Vérification des mods du serveur…",
        M::BusyUpdating => "Mise à jour des mods…",
        M::BusyUpdatingFiles => "Mise à jour des mods, {done} fichiers sur {total}…",
        M::BusyStarting => "Démarrage du serveur…",
        M::BusyStopping => "Arrêt…",

        M::ReadFailed => "Impossible de lire vos serveurs",
        M::SaveFailed => "Impossible d’enregistrer vos serveurs",
        M::NoLongerTrusted => "Vous ne faites plus confiance à {name} pour les mods",
        M::NoLongerTrustedIdChanged => {
            "Un autre core répond désormais à l’adresse {address} (son identifiant a changé). Ne lui refaites confiance sur sa carte que si vous faites confiance à son propriétaire."
        }
        M::NoLongerTrustedOtherCore => "Un autre core répond désormais à l’adresse {address}.",
        M::NoLongerTrustedNewCertificate => {
            "Vous avez approuvé un nouveau certificat pour ce core. Refaites-lui confiance pour les mods sur sa carte si vous êtes sûr qu’il s’agit du même core."
        }
        M::NoLongerTrustedCertificateChanged => {
            "Son certificat a changé. Refaites-lui confiance pour les mods sur sa carte si vous êtes sûr qu’il s’agit du même core."
        }
        M::SignedOutOf => "Déconnecté de {name}",
        M::KeyStillValid => {
            "La clé de cet appareil (lsk_{id}_…) reste valide jusqu’à son expiration. Révoquez-la dans Paramètres → Clés d’API du tableau de bord pour y mettre fin dès maintenant."
        }
        M::KeySaveFailed => "Impossible d’enregistrer la clé dans le {keychain}",
        M::KeySaveFailedError => {
            "Impossible d’enregistrer la clé dans le {keychain}\u{a0}: {error}"
        }
        M::CheckCertificate => "Vérifiez le certificat du core pour continuer.",
        M::CertificateNotTrusted => {
            "Le certificat n’a pas été approuvé, vos identifiants n’ont donc pas été envoyés au core."
        }
        M::CreateFailed => "Impossible de créer l’instance",
        M::CreatedWithoutMods => "{name} a été créée sans les mods du serveur",
        M::CreatedWithoutModsBody => {
            "Le propriétaire n’a pas partagé les mods client de ce serveur\u{202F}; l’instance n’a donc que sa version de Minecraft et son chargeur. Ajoutez vous-même les mods dont elle a besoin."
        }
        M::LinkFailed => "Impossible de lier l’instance au serveur",
        M::ReadyTitle => "{name} est prête",
        M::ReadyBody => {
            "Minecraft {version} avec {loader}, et les mods et configurations du serveur. Appuyez sur Jouer pour le rejoindre."
        }
        M::CheckModsFailed => "Impossible de vérifier les mods du serveur",
        M::JoinFailed => "Impossible de rejoindre {name}",
        M::CoreRemoved => {
            "Le core pour lequel cette instance a été créée ({address}) n’est plus dans Lodestone Launcher. Ajoutez-le sur la page Lodestone Host, ou appuyez sur Jouer sans rejoindre."
        }
        M::ServerNotShown => {
            "Le core ne vous montre plus ce serveur\u{a0}: il a été supprimé, ou il n’est pas listé publiquement et vous n’êtes pas connecté."
        }
        M::NotRunning => "{name} n’est pas en marche",
        M::AskOwnerToStart => {
            "Demandez à son propriétaire de le démarrer, ou connectez-vous au core sur la page Lodestone Host pour le démarrer vous-même."
        }
        M::UpdateModsFailed => "Impossible de mettre à jour les mods",
        M::StopGameFirst => "Quittez d’abord le jeu.",
        M::UpdateInstanceFailed => "Impossible de mettre à jour l’instance",
        M::InstanceNotSaved => {
            "Les mods ont été mis à jour, mais l’instance n’a pas pu être enregistrée\u{a0}: {error}"
        }
        M::BadJoinAddress => "Minecraft ne peut pas rejoindre l’adresse ou le port de ce serveur.",
        M::StartCrashed => {
            "Le serveur a planté au démarrage. Consultez sa console dans le tableau de bord."
        }
        M::StartStopped => {
            "Le serveur s’est arrêté au démarrage. Consultez sa console dans le tableau de bord."
        }
        M::ServerGone => "Le serveur n’existe plus.",
        M::StartTimedOut => "Le serveur n’a pas fini de démarrer en 5\u{a0}minutes.",
        M::StartFailed => "Impossible de démarrer {name}",
        M::StopFailed => "Impossible d’arrêter {name}",
        M::UnlinkFailed => "Impossible de délier l’instance\u{a0}: {error}",
        M::Cancelled => "Annulé",
        M::TrustExplanation => {
            "Les mods d’un Lodestone Core s’exécutent comme des programmes sur {computer}, avec le même accès que vous à vos fichiers et à vos comptes. Ne faites confiance qu’à un core dont le propriétaire a votre confiance."
        }
        M::FingerprintHelp => {
            "Comparez-la à l’empreinte qu’affiche le core\u{a0}: dans le tableau de bord, sous Paramètres → Paramètres du core → Informations («\u{a0}Empreinte du certificat (SHA-256)\u{a0}»), ou dans le journal du core à son démarrage («\u{a0}TLS certificate SHA-256 fingerprint\u{a0}»)."
        }

        M::CoreUnreachable => {
            "Impossible de joindre le core. Vérifiez l’adresse et que le core est en marche. ({detail})"
        }
        M::CoreUntrusted => {
            "Le certificat HTTPS de {host} n’est pas approuvé par {computer}. Vérifiez son empreinte, puis faites-lui confiance pour vous connecter."
        }
        M::CoreCertificateChanged => {
            "Le certificat HTTPS de {host} a changé depuis que vous lui avez fait confiance, Lodestone Launcher refuse donc de s’y connecter. Vérifiez la nouvelle empreinte avant de lui faire confiance."
        }
        M::CoreSignedOut => {
            "Déconnecté\u{a0}: la clé de cet appareil a été révoquée ou a expiré. Reconnectez-vous."
        }
        M::CoreBadCredentials => "Nom d’utilisateur ou mot de passe incorrect.",
        M::CoreForbidden => "Le core a refusé\u{a0}: vous n’avez pas l’autorisation de faire cela.",
        M::CoreNotFound => "Introuvable",
        M::CoreTooOld => {
            "Ce core est trop ancien pour Lodestone Launcher. Mettez à jour Lodestone Core vers une version qui liste les serveurs."
        }
        M::CoreTooManyRequests => "Trop de tentatives. Réessayez dans quelques minutes.",
        M::CoreAnswered => "Le core a répondu {status}\u{a0}: {message}",
        M::CoreInvalidReply => {
            "Le core a envoyé une réponse que Lodestone Launcher ne comprend pas\u{a0}: {detail}"
        }

        M::DifferentMods => "{name} a des mods différents",
        M::DifferentModsBody => {
            "Les mods du serveur ont changé depuis que cette instance les a reçus ({changes}). Mettez-la d’abord à jour pour pouvoir rejoindre le serveur\u{202F}; vos propres mods restent tels quels, et les fichiers remplacés sont conservés dans les sauvegardes de l’instance."
        }
        M::ChangeNew => "Nouveau\u{a0}: {name}",
        M::ChangeChanged => "Modifié\u{a0}: {name}",
        M::ChangeRemoved => "Retiré\u{a0}: {name}",
        M::ChangeConfig => "Configuration\u{a0}: {name}",
        M::NoChanges => "aucun changement",
        M::UpdateAndPlay => "Mettre à jour et jouer",
        M::PlayAnyway => "Jouer quand même",

        M::CertChangedTitle => "Le certificat de {name} a changé",
        M::CertTrustTitle => "Faire confiance au certificat de ce core\u{202F}?",
        M::CertChangedIntro => {
            "Le core à l’adresse {address} présente un autre certificat HTTPS que celui que vous avez approuvé. Le core crée un nouveau certificat quand le sien arrive à expiration ou que son adresse change, mais cela peut aussi vouloir dire que quelqu’un intercepte la connexion."
        }
        M::CertUntrustedIntro => {
            "{os} ne fait pas confiance au certificat HTTPS de {name} à l’adresse {address}. Les Lodestone Cores créent leur propre certificat, c’est donc normal la première fois que vous vous connectez."
        }
        M::FingerprintTrustedBefore => "APPROUVÉ JUSQU’ICI (SHA-256)",
        M::FingerprintPresented => "PRÉSENTÉ MAINTENANT (SHA-256)",
        M::FingerprintCaption => "EMPREINTE DU CERTIFICAT (SHA-256)",
        M::CertIfMatch => {
            "Si elles correspondent, faites-lui confiance\u{a0}: Lodestone Launcher accepte alors exactement ce certificat, seulement pour {host}, et vous avertit s’il change."
        }
        M::CertIfMatchChanged => {
            "Si elles correspondent, faites-lui confiance\u{a0}: Lodestone Launcher accepte alors exactement ce certificat, seulement pour {host}, et vous avertit s’il change. Un core de confiance pour les mods ne l’est plus tant que vous ne lui refaites pas confiance."
        }
        M::TrustNewCertificate => "Approuver le nouveau certificat",
        M::TrustCertificate => "Approuver le certificat",

        M::TrustTitle => "Faites-vous confiance à {name}\u{202F}?",
        M::CoreAt => "{name} à l’adresse {address}.",
        M::TrustAfterAdd => {
            "Tant que vous ne lui faites pas confiance, Lodestone Launcher vous montre les mods d’un serveur avant de les installer. Vous pourrez changer cela plus tard avec «\u{a0}{setting}\u{a0}» sur la carte du core."
        }
        M::TrustLater => {
            "Les mods de ce core seront alors installés sans vous le demander. Vous pourrez désactiver cela sur la carte du core."
        }
        M::TrustThisCore => "Faire confiance à ce core",

        M::AdvisoryTitle => "Les mods de ce serveur vont s’exécuter sur {computer}",
        M::AdvisoryNew => {
            "L’instance pour {server} sur {core} reçoit ces fichiers. Les mods sont des programmes\u{a0}: ils s’exécutent avec le même accès que vous à vos fichiers et à vos comptes."
        }
        M::AdvisoryUpdate => {
            "Passer aux mods de {server} sur {core} installe ces fichiers. Les mods sont des programmes\u{a0}: ils s’exécutent avec le même accès que vous à vos fichiers et à vos comptes."
        }
        M::ModrinthUnreachable => {
            "Impossible de joindre Modrinth, aucun fichier n’a donc pu y être vérifié."
        }
        M::UploadedNotChecked => "Envoyés sur le serveur — non vérifiés par Modrinth ({count})",
        M::AlsoOnModrinth => "Également sur Modrinth ({count})",
        M::AlsoFiles => "S’y ajoutent {files}.",
        M::AlsoFilesScripts => {
            "S’y ajoutent {files}. Les fichiers de script (KubeJS, CraftTweaker) peuvent eux aussi piloter du code."
        }
        M::OnlyIfYouTrust => "Ne continuez que si vous faites confiance au propriétaire de {core}.",
        M::AdvisoryTrustNote => {
            "«\u{a0}{button}\u{a0}» installe aussi ses mods sans vous le demander à partir de maintenant. {explanation}"
        }
        M::InstallThisTime => "Installer cette fois",
        M::TrustAndInstall => "Faire confiance à {core} et installer",

        M::KeyDays90 => "90\u{a0}jours",
        M::KeyYear => "1\u{a0}an",
        M::KeyNever => "Jamais",
        M::AddressPlaceholder => "192.168.1.20 ou localhost:16662",
        M::UsernameOptional => "Nom d’utilisateur (facultatif)",
        M::Username => "Nom d’utilisateur",
        M::Password => "Mot de passe",
        M::EnterAddress => "Saisissez l’adresse du core.",
        M::EnterUsernamePassword => "Saisissez votre nom d’utilisateur et votre mot de passe.",
        M::EnterPassword => "Saisissez votre mot de passe, ou laissez le nom d’utilisateur vide.",
        M::ConsoleAccess => "Accès à la console (facultatif)",
        M::ReadConsole => "Lire la console",
        M::SendChat => "Envoyer des messages et des commandes",
        M::ConsoleAccessHint => {
            "Seulement là où votre utilisateur peut utiliser la console. Les commandes peuvent faire tout ce que peut un opérateur\u{a0}: ne les autorisez que si vous faites confiance à tous ceux qui utilisent {computer}."
        }
        M::SignInTitle => "Se connecter au serveur",
        M::SignInIntro => {
            "Connectez-vous pour démarrer et arrêter des serveurs et voir ceux qui ne sont pas listés publiquement. Lodestone Launcher crée sa propre clé pour {computer}\u{202F}; votre mot de passe n’est pas conservé."
        }
        M::AddIntro => {
            "L’adresse d’un Lodestone Core. Sans vous connecter, vous voyez les serveurs que son propriétaire liste publiquement."
        }
        M::Address => "Adresse",
        M::SignInOptional => "Connexion (facultatif)",
        M::KeyExpiresAfter => "La clé de {computer} expire après",
        M::NeverExpiresWarning => {
            "Une clé qui n’expire jamais fonctionne jusqu’à ce que vous la révoquiez dans le tableau de bord."
        }

        M::ChatPlaceholder => "Dites quelque chose, ou /commande pour en exécuter une",
        M::ConsoleNotAllowedAtSignIn => {
            "La lecture de la console n’a pas été autorisée lors de la connexion de {computer}."
        }
        M::ConsoleKeyCantRead => {
            "La clé de {computer} ne peut pas lire la console. Reconnectez-vous et autorisez «\u{a0}{option}\u{a0}», si votre utilisateur en a le droit."
        }
        M::ConsoleKeyCantReadShort => "La clé de {computer} ne peut pas lire la console.",
        M::ConsoleKeyCantWrite => {
            "La clé de {computer} ne peut pas envoyer de messages ni de commandes. Reconnectez-vous et autorisez «\u{a0}{option}\u{a0}», si votre utilisateur peut utiliser la console."
        }
        M::OfflineCantLink => {
            "Les comptes hors ligne ne peuvent pas être liés\u{a0}: leur UUID vient du nom, il ne prouve donc rien."
        }
        M::LinkedTo => "{account} est lié à {user}.",
        M::YourLodestoneUser => "votre utilisateur Lodestone",
        M::Connecting => "Connexion…",
        M::Live => "En direct",
        M::Reconnecting => "Reconnexion\u{a0}: {reason}",
        M::ConsoleNotShown => "La console n’est pas affichée",
        M::OutputAppearsHere => "La sortie du serveur s’affiche ici.",
        M::Send => "Envoyer",
        M::ChatHint => {
            "Le texte est dit à tous avec /say. Commencez par / pour exécuter une commande."
        }
        M::Console => "Console",
        M::JumpToLatest => "Aller à la fin",
        M::Players => "Joueurs",
        M::NoOneOnline => "Personne n’est en ligne.",
        M::ServerNotRunning => "Le serveur n’est pas en marche.",
        M::MinecraftAccount => "Compte Minecraft",
        M::CoreTooOldToLink => "Ce core est trop ancien pour lier des comptes Minecraft.",
        M::Unlink => "Délier",
        M::LinkAccount => "Lier {account} à {user}",
        M::LinkCheckHint => {
            "Le core vérifie auprès de Microsoft que le compte est bien à vous, en utilisant cette connexion une seule fois. Il ne la conserve pas."
        }
        M::SignInMicrosoftToLink => {
            "Connectez-vous avec un compte Microsoft pour le lier à votre utilisateur Lodestone."
        }

        M::HostTitle => "Héberger sur un Lodestone Core",
        M::ShowOnServersPage => "Afficher sur la page Lodestone Host",
        M::HostIntro => {
            "Crée un serveur à partir des mods et des configurations de {name}, sous forme de modpack Modrinth. Les mods Modrinth réservés au client sont exclus. Le serveur est créé arrêté."
        }
        M::CoreLabel => "Core",
        M::PasswordNotKept => {
            "Votre mot de passe est envoyé une seule fois pour vous connecter, et n’est pas conservé."
        }
        M::ServerName => "Nom du serveur",
        M::Memory => "Mémoire",
        M::GamePortHint => {
            "Le port du jeu est le premier libre à partir de {port} sur la machine du core."
        }
        M::EulaAgree => {
            "J’accepte le CLUF de Minecraft ({url}). Lodestone Core l’accepte pour le nouveau serveur."
        }
        M::ReadEula => "Lire le CLUF",
        M::CreateServer => "Créer le serveur",
        M::NameTheServer => "Donnez un nom au serveur.",
        M::HostNeedsPassword => {
            "Saisissez votre nom d’utilisateur et votre mot de passe pour ce core. Créer un serveur demande plus que ce que la clé de {computer} peut faire."
        }
        M::AgreeEulaFirst => {
            "Acceptez d’abord le CLUF de Minecraft\u{a0}: le core l’accepte pour le nouveau serveur."
        }
        M::StageExporting => "Exportation de l’instance…",
        M::StageSigningIn => "Connexion…",
        M::StageUploading => "Envoi du modpack ({size})…",
        M::StageCreating => "Création du serveur…",
        M::CoreGone => "Ce core n’existe plus.",
        M::InstanceGone => "L’instance n’existe plus.",
        M::PackNotKept => "Le core n’a pas conservé le modpack.",
        M::SignedOutRightAway => "Le core vous a déconnecté juste après la connexion. Réessayez.",
        M::AddCoreFirst => "Ajoutez d’abord votre Lodestone Core sur la page Lodestone Host.",

        M::LanListenFailed => "Impossible d’écouter les mondes en LAN\u{a0}: {error}",
        M::LanLooking => "Recherche de mondes ouverts en LAN…",
        M::LanNoneMac => {
            "Aucun monde en LAN trouvé. Dans Minecraft, ouvrez un monde et choisissez Ouvrir au LAN. Si macOS a demandé si Lodestone Launcher pouvait trouver des appareils sur votre réseau local, autorisez-le dans Réglages Système → Confidentialité et sécurité → Réseau local, puis rouvrez cette page."
        }
        M::LanNoneWindows => {
            "Aucun monde en LAN trouvé. Dans Minecraft, ouvrez un monde et choisissez Ouvrir au LAN. Si Windows a demandé s’il fallait laisser passer Lodestone Launcher par son pare-feu, autorisez-le sur les réseaux privés, puis rouvrez cette page."
        }
        M::LanNoneOther => {
            "Aucun monde en LAN trouvé. Dans Minecraft, ouvrez un monde et choisissez Ouvrir au LAN, puis rouvrez cette page."
        }
        M::LanWorld => "Monde en LAN",
        M::Join => "Rejoindre",
        M::OnYourNetwork => "Sur votre réseau",
        M::OnYourNetworkHint => "Les mondes ouverts en LAN sur ce réseau.",
        M::JoinWorld => "Rejoindre {world}",
        M::PickInstance => {
            "Choisissez l’instance avec laquelle jouer. Elle doit avoir la même version de Minecraft et les mêmes mods que le monde à l’adresse {address}."
        }
        M::NoInstancesYet => "Vous n’avez pas encore d’instance. Créez-en une d’abord.",

        M::ANewInstance => "une nouvelle instance",
        M::NewMatchingInstance => "Nouvelle instance correspondante",
        M::NoResourcePack => "Ce serveur ne définit pas de pack de ressources.",
        M::DownloadToInstance => "Télécharger dans l’instance",
        M::PackNote => {
            "Minecraft le télécharge quand vous rejoignez le serveur\u{202F}; l’enregistrer dans {target} le rend aussi disponible hors ligne."
        }
        M::PackNoteRequired => {
            "Les joueurs doivent l’accepter pour jouer. Minecraft le télécharge quand vous rejoignez le serveur\u{202F}; l’enregistrer dans {target} le rend aussi disponible hors ligne."
        }
        M::PackNoteNoSha => {
            "Le serveur ne donne pas de SHA-1 pour ce pack. Minecraft le télécharge quand vous rejoignez le serveur\u{202F}; l’enregistrer dans {target} le rend aussi disponible hors ligne."
        }
        M::PackNoteRequiredNoSha => {
            "Les joueurs doivent l’accepter pour jouer. Le serveur ne donne pas de SHA-1 pour ce pack. Minecraft le télécharge quand vous rejoignez le serveur\u{202F}; l’enregistrer dans {target} le rend aussi disponible hors ligne."
        }
        M::InstanceLabel => "Instance",
        M::ResourcePack => "Pack de ressources",
        M::World => "Monde",
        M::WorldCopyHint => {
            "Une copie solo du monde du serveur, dans un nouveau dossier de monde. Les modifications que vous y faites restent sur {computer}."
        }
        M::StopToDownload => {
            "Arrêtez le serveur pour télécharger son monde\u{a0}: un serveur en marche peut être en train d’y écrire."
        }
        M::PasswordNotKeptDownload => {
            "Votre mot de passe est envoyé une seule fois pour vous connecter pour ce téléchargement, et n’est pas conservé."
        }
        M::DownloadWorld => "Télécharger le monde",
        M::WorldNeedsPassword => {
            "Saisissez votre nom d’utilisateur et votre mot de passe pour ce core. Télécharger un monde demande l’autorisation de lire les fichiers du serveur, que la clé de {computer} n’a pas."
        }
        M::SignInToSeePack => {
            "Connectez-vous à ce core pour voir le pack de ressources du serveur."
        }
        M::CreatingMatching => "Création de l’instance correspondante",
        M::CreatingMatchingBody => {
            "Elle sera créée une fois les mods du serveur téléchargés. Ouvrez alors de nouveau {more} sur le serveur, choisissez-la, et relancez le téléchargement."
        }
        M::CouldNotCreateInstance => "Impossible de créer l’instance.",
        M::QuitGameFirst => "Quittez d’abord le jeu dans cette instance.",
        M::WorldZipping => "Le core compresse le monde…",
        M::WorldDownloading => "Téléchargement…",
        M::WorldDownloadingOf => "Téléchargement\u{a0}: {done} sur {total}",
        M::WorldDownloadingSize => "Téléchargement\u{a0}: {done}",
        M::WorldImporting => "Importation…",
        M::WorldImportingOf => "Importation\u{a0}: {done} sur {total}",
        M::WorldImported => {
            "Importé comme le monde «\u{a0}{folder}\u{a0}» dans {instance}. Ouvrez-le depuis Solo\u{202F}; c’est une copie, les modifications ne reviennent donc pas sur le serveur."
        }
        M::FriendsTitle => "Serveurs d’amis",
        M::FriendsHint => {
            "Les serveurs que vos amis partagent avec vous grâce aux liens d’invitation Lodestone Connect."
        }
        M::JoinAFriend => "Rejoindre un ami",
        M::NoFriendsYet => {
            "Quand un ami partage un serveur depuis Lodestone Host, il obtient un lien d’invitation. Ouvrez-le, ou collez-le ici avec Rejoindre un ami."
        }
        M::SharedBy => "Partagé par {name}",
        M::FriendCheck => "Vérifier",
        M::FriendChecking => "Vérification…",
        M::FriendPlayingNow => "En jeu",
        M::StopTrusting => "Ne plus faire confiance",
        M::ConnectedVia => "Connexion directe {how}, {ms}\u{a0}ms",
        M::ViaSameComputer => "sur le même ordinateur",
        M::ViaLocalNetwork => "par votre réseau local",
        M::ViaIpv6 => "par IPv6",
        M::ViaInternet => "par Internet",
        M::JoinFriendTitle => "Rejoindre le serveur d’un ami",
        M::JoinFriendIntro => {
            "Collez le lien d’invitation que votre ami vous a envoyé. Il commence par https://connect.lodestone.cc/j/."
        }
        M::InvitePlaceholder => "Lien d’invitation (https://connect.lodestone.cc/j/…)",
        M::NotAnInvite => {
            "Ce n’est pas un lien d’invitation Lodestone. Les liens d’invitation commencent par https://connect.lodestone.cc/j/."
        }
        M::PasteInviteFirst => "Collez d’abord un lien d’invitation.",
        M::ConnectingTo => "Connexion à {name}…",
        M::ConnectingToFriend => "Connexion au serveur de votre ami…",
        M::StageFinding => {
            "Recherche d’un chemin direct entre votre réseau et celui de votre ami. Cela prend quelques secondes."
        }
        M::StageFindingSlow => {
            "Toujours en recherche. Certains réseaux demandent jusqu’à une minute."
        }
        M::StageReading => "Connecté. Lecture des informations du serveur…",
        M::FactVersion => "Version",
        M::FactMods => "Mods",
        M::FactStatus => "État",
        M::FactConnection => "Connexion",
        M::FactInstance => "Instance",
        M::FriendHasMods => {
            "Ce serveur utilise des mods. Lodestone Launcher installe les mêmes pour vous."
        }
        M::FriendNoMods => "Aucun n’est nécessaire",
        M::PlaysWithLinked => {
            "{name}, créée pour ce serveur. Ses mods restent à jour avec ceux du serveur."
        }
        M::PlaysWithInstance => "Votre instance {name}, qui a la même version.",
        M::MakesInstance => "Jouer crée une nouvelle instance de la même version.",
        M::MakesInstanceWithMods => {
            "Jouer crée une nouvelle instance avec la version et les mods du serveur."
        }
        M::FriendServerStopped => "Le serveur est arrêté",
        M::FriendServerStoppedBody => {
            "Demandez à votre ami de le démarrer. Minecraft ne peut pas le rejoindre tant qu’il ne tourne pas."
        }
        M::FriendStarting => "Préparation du jeu…",
        M::FriendInstanceBusy => "Cette instance est déjà lancée. Quittez d’abord le jeu.",
        M::FriendLinkEnded => "Votre ami a arrêté de partager ce serveur",
        M::FriendLinkEndedBody => {
            "Demandez-lui un nouveau lien."
        }
        M::FriendNotSharing => "Votre ami ne partage pas ce serveur en ce moment",
        M::FriendNotSharingBody => {
            "Son ordinateur ou Lodestone Host est peut-être éteint. Réessayez plus tard, ou demandez à votre ami de reprendre le partage."
        }
        M::FriendNoPath => {
            "Votre réseau et celui de votre ami ne peuvent pas se connecter directement"
        }
        M::FriendNoPathBody => {
            "Lodestone Connect relie directement deux ordinateurs, et ces deux réseaux l’empêchent. Essayez un autre réseau (le partage de connexion d’un téléphone, par exemple), ou demandez à votre ami d’ouvrir un port sur son routeur."
        }
        M::FriendWrongHost => "Ce n’est pas le serveur de votre ami",
        M::FriendWrongHostBody => {
            "L’ordinateur qui a répondu n’est pas celui pour lequel ce lien a été créé\u{202F}: Lodestone Launcher s’est donc arrêté avant d’envoyer quoi que ce soit. Quelqu’un se fait peut-être passer pour le serveur de votre ami. Demandez-lui un nouveau lien, et n’utilisez que des liens reçus directement de lui."
        }
        M::FriendOffline => "Cet ordinateur semble hors ligne",
        M::FriendOfflineBody => "Vérifiez votre connexion Internet, puis réessayez.",
        M::FriendServiceUnreachable => "Impossible de joindre Lodestone Connect",
        M::FriendServiceUnreachableBody => {
            "connect.lodestone.cc n’a pas répondu ({detail}). Vérifiez votre connexion Internet, puis réessayez."
        }
        M::FriendJoinFailed => "Impossible de rejoindre le serveur",
        M::FriendJoinFailedBody => "Un problème est survenu\u{a0}: {detail}",
        M::FriendTunnelEnded => "La connexion à {name} a pris fin",
        M::FriendTunnelEndedBody => {
            "Votre ami a arrêté le partage, ou son ordinateur s’est déconnecté. Minecraft indique que la connexion a été perdue. Rejouez quand il sera de retour."
        }
        M::FriendLinkMissing => {
            "Lodestone Launcher n’a pas pu lire le lien d’invitation de ce serveur. Ouvrez de nouveau le lien reçu de votre ami."
        }
        M::FriendRemoved => {
            "Cette instance a été créée pour le serveur d’un ami ({link}) qui n’est plus dans Lodestone Launcher. Ouvrez de nouveau son lien d’invitation pour le rejoindre."
        }
        M::FriendsReadFailed => "Impossible de lire les serveurs de vos amis",
        M::FriendsSaveFailed => "Impossible d’enregistrer les serveurs de vos amis",
        M::EulaNotAccepted => "Le CLUF de Minecraft n’est pas accepté pour ce serveur",
        M::EulaNotAcceptedBody => {
            "Le serveur ne peut pas démarrer tant que quelqu’un n’a pas accepté le CLUF de Minecraft de Mojang ({url}). Lisez-le, et ne l’acceptez que si vous êtes d’accord."
        }
        M::AcceptEula => "Accepter le CLUF de Minecraft",
        M::EulaAccepted => {
            "Vous avez accepté le CLUF de Minecraft pour ce serveur. Il peut démarrer maintenant."
        }
        M::EulaAcceptFailed => "Impossible d’accepter le CLUF\u{a0}: {error}",
        M::PackSaved => {
            "Enregistré sous {file} dans les packs de ressources de l’instance. Activez-le là-bas pour l’utiliser aussi sur d’autres serveurs."
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Players => ["{count} joueur", "{count} joueurs"],
        P::ReadyBodySkipped => [
            "Minecraft {version} avec {loader}, et les mods et configurations du serveur. {count} fichier du pack a été laissé de côté. Appuyez sur Jouer pour le rejoindre.",
            "Minecraft {version} avec {loader}, et les mods et configurations du serveur. {count} fichiers du pack ont été laissés de côté. Appuyez sur Jouer pour le rejoindre.",
        ],
        P::ModsAdded => ["{count} ajouté", "{count} ajoutés"],
        P::ModsUpdated => ["{count} mis à jour", "{count} mis à jour"],
        P::ModsRemoved => ["{count} retiré", "{count} retirés"],
        P::OtherFiles => ["{count} autre fichier", "{count} autres fichiers"],
        P::ConfigFiles => [
            "{count} fichier de configuration",
            "{count} fichiers de configuration",
        ],
        P::ScriptFiles => ["{count} fichier de script", "{count} fichiers de script"],
        P::AndMore => ["et {count} de plus", "et {count} de plus"],
        P::HostDone => [
            "{name} est sur {core}, sur le port {port}. Le core le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichier de mod depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre.",
            "{name} est sur {core}, sur le port {port}. Le core le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichiers de mods depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre.",
        ],
        P::HostDoneRelinked => [
            "{name} est sur {core}, sur le port {port}. Le core le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichier de mod depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre. Cette instance était liée à {server}\u{202F}; elle appartient désormais au nouveau serveur.",
            "{name} est sur {core}, sur le port {port}. Le core le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichiers de mods depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre. Cette instance était liée à {server}\u{202F}; elle appartient désormais au nouveau serveur.",
        ],
    }
}
