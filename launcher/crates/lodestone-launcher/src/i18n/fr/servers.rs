use crate::i18n::{ServersMsg as M, ServersPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ServersTitle => "Lodestone Host",
        M::FriendsPageTitle => "Amis",
        M::OnThisComputer => "Sur cet ordinateur",
        M::Rescan => "Rechercher à nouveau",
        M::ScanningLocal => "Recherche d’hôtes sur cet ordinateur…",
        M::NoLocalCores => "Aucun hôte ne tourne sur cet ordinateur.",
        M::HostOfferTitle => "Hébergez n’importe quel serveur et jouez avec vos amis tout de suite",
        M::HostOfferBody => "Rien à configurer, aucune redirection de port.",
        M::HostOfferDownload => "Télécharger Lodestone Host",
        M::HostOfferCoreOnly => "Télécharger seulement le programme serveur",
        M::ConnectAsOwner => "Se connecter en tant que propriétaire",
        M::ChooseDataFolder => "Choisir le dossier…",
        M::ChooseDataFolderPrompt => "Choisir",
        M::ProductHostHint => "l’application de bureau, avec le tableau de bord dans une fenêtre",
        M::ProductCoreHint => "le programme serveur seul, ou Docker",
        M::ConnectedAsOwner => "Connecté à {name} en tant que propriétaire",
        M::ConnectedMadeOwner => {
            "Il n’avait pas encore de propriétaire\u{202F}: Lodestone Launcher a créé le compte propriétaire {username}."
        }
        M::ConnectedOwnerKey => {
            "Ce lanceur a maintenant une clé de propriétaire pour cet hôte. Vous pouvez la révoquer depuis le tableau de bord de l’hôte."
        }
        M::StartedBy => "Démarré par {who}",
        M::StartedDirectly => "Démarré directement",
        M::DirectNote => {
            "Cet hôte a été démarré directement. Il est fait pour être lancé avec lodestone_cli, qui l’installe et le garde à jour."
        }
        M::GetLodestoneCli => "Obtenir lodestone_cli",
        M::CredentialsReadFailed => {
            "Impossible de lire les clés de propriétaire des hôtes de cet ordinateur"
        }
        M::CredentialsSaveFailed => "Impossible d’enregistrer la clé de propriétaire",
        M::ClaimNoCore => "Aucun hôte ne tourne sur cet ordinateur.",
        M::ClaimRefused => {
            "L’hôte a refusé l’accès propriétaire. Activez «\u{a0}Laisser les applications de cet ordinateur obtenir l’accès propriétaire\u{a0}» dans ses paramètres, ou mettez-le à jour."
        }
        M::ClaimFolderNotFound => {
            "Impossible de trouver le dossier de données de cet hôte. Choisissez le dossier où il garde ses données (son LODESTONE_PATH). Un hôte dans Docker doit avoir son dossier de données monté depuis cet ordinateur."
        }
        M::ClaimNotThisCore => {
            "L’hôte qui a répondu n’est pas celui dont le dossier a été trouvé\u{202F}: rien n’a été enregistré."
        }
        M::ClaimNoResult => "L’hôte n’a pas terminé la demande. Réessayez.",
        M::ClaimNoCertificate => {
            "Cet hôte n’a pas de certificat HTTPS\u{202F}: Lodestone Launcher ne peut donc pas garantir que sa clé de propriétaire n’ira qu’à lui. Activez HTTPS dans les paramètres de l’hôte, puis reconnectez-vous."
        }
        M::ClaimFiles => "Impossible d’utiliser le dossier de données de l’hôte\u{a0}: {error}",
        M::ClaimInvalid => {
            "L’hôte a envoyé quelque chose que Lodestone Launcher ne comprend pas\u{a0}: {error}"
        }
        M::AddServer => "Ajouter un hôte",
        M::Dismiss => "Masquer",
        M::OtherCoresTitle => "Autres hôtes et serveurs",
        M::OtherCoresHint => {
            "Les hôtes d’autres ordinateurs ajoutés par leur adresse, et les serveurs rejoints avec un lien d’invitation."
        }
        M::NoOtherCores => "Aucun autre hôte ni serveur pour l’instant",
        M::NoServersHint => {
            "Ajoutez un hôte par son adresse, ou un serveur avec le lien d’invitation qu’on vous a envoyé."
        }

        M::CoreMoreActions => "Plus d’actions pour {name}",
        M::CopyOwnerApiKey => "Copier la clé d’API de propriétaire",
        M::OwnerKeyCopied => "Clé d’API de propriétaire de {name} copiée",
        M::OwnerKeyCopiedHint => {
            "Gardez-la secrète\u{202F}: toute personne qui l’a peut contrôler cet hôte en tant que propriétaire."
        }
        M::CertificatePinned => "Certificat épinglé",
        M::SignedInAs => "Connecté en tant que {username}",
        M::SignedIn => "Connecté",
        M::NotSignedIn => "Non connecté\u{a0}: serveurs publics uniquement",
        M::TrustedForMods => "Confiance pour les mods",
        M::Offline => {
            "Hors ligne\u{a0}: impossible de joindre cet hôte. Vérifiez qu’il est en marche et sur le même réseau."
        }
        M::NotListedPublicly => {
            "Le propriétaire n’a pas activé «\u{a0}Lister les serveurs publiquement\u{a0}» dans les Paramètres du core, il n’y a donc rien à afficher sans vous connecter."
        }
        M::NoPublicServers => {
            "Aucun serveur n’est listé publiquement. Connectez-vous pour voir ceux auxquels vous avez accès."
        }
        M::NoVisibleServers => "Aucun serveur visible pour vous sur cet hôte.",
        M::CertChangedWarning => {
            "Le certificat HTTPS de cet hôte a changé depuis que vous lui avez fait confiance, Lodestone Launcher refuse donc de s’y connecter. L’hôte crée un nouveau certificat quand le sien arrive à expiration ou que son adresse change\u{202F}; cela peut aussi vouloir dire que quelqu’un intercepte la connexion. Vérifiez la nouvelle empreinte avant de lui faire confiance."
        }
        M::CertUntrustedWarning => {
            "Le certificat HTTPS de cet hôte n’est pas approuvé par {computer}. Les hôtes créent leur propre certificat, c’est donc normal\u{a0}: vérifiez son empreinte, puis faites-lui confiance."
        }
        M::ReviewNewCertificate => "Approuver le nouveau certificat…",
        M::ReviewCertificate => "Approuver le certificat…",

        M::CreateInstance => "Créer l’instance",
        M::StartAndJoin => "Démarrer et rejoindre",
        M::Play => "Jouer et rejoindre",
        M::SharedTag => "Partagé",
        M::CopyInviteLink => "Copier le lien d’invitation",
        M::InviteLinkCopied => "Lien d’invitation de {name} copié",
        M::InviteLinkCopiedHint => {
            "Toute personne qui l’a peut rejoindre tant que le serveur est partagé."
        }
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
            "Un autre hôte répond désormais à l’adresse {address} (son identifiant a changé). Ne lui refaites confiance sur sa carte que si vous faites confiance à son propriétaire."
        }
        M::NoLongerTrustedOtherCore => "Un autre hôte répond désormais à l’adresse {address}.",
        M::NoLongerTrustedNewCertificate => {
            "Vous avez approuvé un nouveau certificat pour cet hôte. Refaites-lui confiance pour les mods sur sa carte si vous êtes sûr qu’il s’agit du même hôte."
        }
        M::NoLongerTrustedCertificateChanged => {
            "Son certificat a changé. Refaites-lui confiance pour les mods sur sa carte si vous êtes sûr qu’il s’agit du même hôte."
        }
        M::SignedOutOf => "Déconnecté de {name}",
        M::KeyStillValid => {
            "La clé de cet appareil (lsk_{id}_…) reste valide jusqu’à son expiration. Révoquez-la dans Paramètres → Clés d’API du tableau de bord pour y mettre fin dès maintenant."
        }
        M::KeySaveFailed => "Impossible d’enregistrer la clé dans le {keychain}",
        M::KeySaveFailedError => {
            "Impossible d’enregistrer la clé dans le {keychain}\u{a0}: {error}"
        }
        M::CheckCertificate => "Vérifiez le certificat de l’hôte pour continuer.",
        M::CertificateNotTrusted => {
            "Le certificat n’a pas été approuvé, vos identifiants n’ont donc pas été envoyés à l’hôte."
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
            "L’hôte pour lequel cette instance a été créée ({address}) n’est plus dans Lodestone Launcher. Ajoutez-le sur la page Lodestone Host, ou appuyez sur Jouer sans rejoindre."
        }
        M::ServerNotShown => {
            "L’hôte ne vous montre plus ce serveur\u{a0}: il a été supprimé, ou il n’est pas listé publiquement et vous n’êtes pas connecté."
        }
        M::NotRunning => "{name} n’est pas en marche",
        M::AskOwnerToStart => {
            "Demandez à son propriétaire de le démarrer, ou connectez-vous à l’hôte sur la page Lodestone Host pour le démarrer vous-même."
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
            "Les mods d’un hôte s’exécutent comme des programmes sur {computer}, avec le même accès que vous à vos fichiers et à vos comptes. Ne faites confiance qu’à un hôte dont le propriétaire a votre confiance."
        }
        M::FingerprintHelp => {
            "Comparez-la à l’empreinte qu’affiche l’hôte\u{a0}: dans le tableau de bord, sous Paramètres → Paramètres du core → Informations («\u{a0}Empreinte du certificat (SHA-256)\u{a0}»), ou dans le journal de l’hôte à son démarrage («\u{a0}TLS certificate SHA-256 fingerprint\u{a0}»)."
        }

        M::CoreUnreachable => {
            "Impossible de joindre l’hôte. Vérifiez l’adresse et que l’hôte est en marche. ({detail})"
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
        M::CoreForbidden => "L’hôte a refusé\u{a0}: vous n’avez pas l’autorisation de faire cela.",
        M::CoreNotFound => "Introuvable",
        M::CoreTooOld => {
            "Cet hôte est trop ancien pour Lodestone Launcher. Mettez-le à jour vers une version qui liste les serveurs."
        }
        M::CoreTooManyRequests => "Trop de tentatives. Réessayez dans quelques minutes.",
        M::CoreAnswered => "L’hôte a répondu {status}\u{a0}: {message}",
        M::CoreInvalidReply => {
            "L’hôte a envoyé une réponse que Lodestone Launcher ne comprend pas\u{a0}: {detail}"
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
        M::CertTrustTitle => "Faire confiance au certificat de cet hôte\u{202F}?",
        M::CertChangedIntro => {
            "L’hôte à l’adresse {address} présente un autre certificat HTTPS que celui que vous avez approuvé. L’hôte crée un nouveau certificat quand le sien arrive à expiration ou que son adresse change, mais cela peut aussi vouloir dire que quelqu’un intercepte la connexion."
        }
        M::CertUntrustedIntro => {
            "{os} ne fait pas confiance au certificat HTTPS de {name} à l’adresse {address}. Les hôtes créent leur propre certificat, c’est donc normal la première fois que vous vous connectez."
        }
        M::FingerprintTrustedBefore => "APPROUVÉ JUSQU’ICI (SHA-256)",
        M::FingerprintPresented => "PRÉSENTÉ MAINTENANT (SHA-256)",
        M::FingerprintCaption => "EMPREINTE DU CERTIFICAT (SHA-256)",
        M::CertIfMatch => {
            "Si elles correspondent, faites-lui confiance\u{a0}: Lodestone Launcher accepte alors exactement ce certificat, seulement pour {host}, et vous avertit s’il change."
        }
        M::CertIfMatchChanged => {
            "Si elles correspondent, faites-lui confiance\u{a0}: Lodestone Launcher accepte alors exactement ce certificat, seulement pour {host}, et vous avertit s’il change. Un hôte de confiance pour les mods ne l’est plus tant que vous ne lui refaites pas confiance."
        }
        M::TrustNewCertificate => "Approuver le nouveau certificat",
        M::TrustCertificate => "Approuver le certificat",

        M::TrustTitle => "Faites-vous confiance à {name}\u{202F}?",
        M::CoreAt => "{name} à l’adresse {address}.",
        M::TrustAfterAdd => {
            "Tant que vous ne lui faites pas confiance, Lodestone Launcher vous montre les mods d’un serveur avant de les installer. Vous pourrez changer cela plus tard avec «\u{a0}{setting}\u{a0}» sur la carte de l’hôte."
        }
        M::TrustLater => {
            "Les mods de cet hôte seront alors installés sans vous le demander. Vous pourrez désactiver cela sur la carte de l’hôte."
        }
        M::TrustThisCore => "Faire confiance à cet hôte",

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
        M::EnterAddress => "Saisissez l’adresse de l’hôte.",
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
            "L’adresse d’un hôte. Sans vous connecter, vous voyez les serveurs que son propriétaire liste publiquement."
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
        M::MinecraftAccount => "Compte Minecraft",
        M::CoreTooOldToLink => "Cet hôte est trop ancien pour lier des comptes Minecraft.",
        M::Unlink => "Délier",
        M::LinkAccount => "Lier {account} à {user}",
        M::LinkCheckHint => {
            "L’hôte vérifie auprès de Microsoft que le compte est bien à vous, en utilisant cette connexion une seule fois. Il ne la conserve pas."
        }
        M::SignInMicrosoftToLink => {
            "Connectez-vous avec un compte Microsoft pour le lier à votre utilisateur Lodestone."
        }

        M::HostTitle => "Héberger sur Lodestone",
        M::ShowOnServersPage => "Afficher sur la page Lodestone Host",
        M::HostIntro => {
            "Crée un serveur à partir des mods et des configurations de {name}, sous forme de modpack Modrinth. Les mods Modrinth réservés au client sont exclus. Le serveur est créé arrêté."
        }
        M::CoreLabel => "Hôte",
        M::PasswordNotKept => {
            "Votre mot de passe est envoyé une seule fois pour vous connecter, et n’est pas conservé."
        }
        M::ServerName => "Nom du serveur",
        M::Memory => "Mémoire",
        M::GamePortHint => {
            "Le port du jeu est le premier libre à partir de {port} sur la machine de l’hôte."
        }
        M::EulaAgree => {
            "J’accepte le CLUF de Minecraft ({url}). L’hôte l’accepte pour le nouveau serveur."
        }
        M::ReadEula => "Lire le CLUF",
        M::CreateServer => "Créer le serveur",
        M::NameTheServer => "Donnez un nom au serveur.",
        M::HostNeedsPassword => {
            "Saisissez votre nom d’utilisateur et votre mot de passe pour cet hôte. Créer un serveur demande plus que ce que la clé de {computer} peut faire."
        }
        M::AgreeEulaFirst => {
            "Acceptez d’abord le CLUF de Minecraft\u{a0}: l’hôte l’accepte pour le nouveau serveur."
        }
        M::StageExporting => "Exportation de l’instance…",
        M::StageSigningIn => "Connexion…",
        M::StageUploading => "Envoi du modpack ({size})…",
        M::StageCreating => "Création du serveur…",
        M::CoreGone => "Cet hôte n’existe plus.",
        M::InstanceGone => "L’instance n’existe plus.",
        M::PackNotKept => "L’hôte n’a pas conservé le modpack.",
        M::SignedOutRightAway => "L’hôte vous a déconnecté juste après la connexion. Réessayez.",
        M::AddCoreFirst => "Ajoutez d’abord votre hôte sur la page Lodestone Host.",

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

        M::FriendsTitle => "Serveurs des liens d’invitation",
        M::FriendsHint => {
            "Les serveurs qu’on vous a partagés par un lien d’invitation Lodestone Connect."
        }
        M::JoinAFriend => "Ajouter un serveur",
        M::SharedBy => "Partagé par {name}",
        M::SharedWithFriends => "Partagé avec des amis",
        M::SharedWithFriendsHint => "Vous les avez rejoints avec leur lien d’invitation.",
        M::ConnectToJoinDirectly => {
            "Connectez-vous en tant que propriétaire pour le rejoindre directement, sans tunnel."
        }
        M::DirectConnection => "Directement sur cet ordinateur, sans tunnel",
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
        M::FriendLinkEndedBody => "Demandez-lui un nouveau lien.",
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
        M::FriendAddress => "Magic localhost\u{a0}: {address} (une fois connecté)",
        M::FriendPortTaken => "Le port {port} était occupé",
        M::FriendPortTakenBody => {
            "Un autre programme de cet ordinateur l’utilise, donc cette fois l’adresse est {address}. La prochaine fois, Lodestone Launcher essaiera de nouveau le port {port}."
        }
        M::EulaNotAccepted => "Le CLUF de Minecraft n’est pas accepté pour ce serveur",
        M::EulaNotAcceptedBody => {
            "Le serveur ne peut pas démarrer tant que quelqu’un n’a pas accepté le CLUF de Minecraft de Mojang ({url}). Lisez-le, et ne l’acceptez que si vous êtes d’accord."
        }
        M::AcceptEula => "Accepter le CLUF de Minecraft",
        M::EulaAccepted => {
            "Vous avez accepté le CLUF de Minecraft pour ce serveur. Il peut démarrer maintenant."
        }
        M::EulaAcceptFailed => "Impossible d’accepter le CLUF\u{a0}: {error}",
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
            "{name} est sur {core}, sur le port {port}. L’hôte le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichier de mod depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre.",
            "{name} est sur {core}, sur le port {port}. L’hôte le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichiers de mods depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre.",
        ],
        P::HostDoneRelinked => [
            "{name} est sur {core}, sur le port {port}. L’hôte le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichier de mod depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre. Cette instance était liée à {server}\u{202F}; elle appartient désormais au nouveau serveur.",
            "{name} est sur {core}, sur le port {port}. L’hôte le prépare maintenant\u{a0}: il installe Fabric et télécharge {count} fichiers de mods depuis Modrinth.\n\nLe serveur est arrêté. Démarrez-le depuis la page Lodestone Host ou le tableau de bord quand vous êtes prêt\u{202F}; cette instance est prête à le rejoindre. Cette instance était liée à {server}\u{202F}; elle appartient désormais au nouveau serveur.",
        ],
    }
}
