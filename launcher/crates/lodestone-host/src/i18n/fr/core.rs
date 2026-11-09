use crate::i18n::{CoreMsg as M, CorePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::SectionCore => "Core",
        M::SectionPublic => "Serveurs publics",
        M::SectionPublicLead => "Visible sans connexion.",
        M::SectionInfo => "Informations",
        M::SectionDanger => "Zone de danger",
        M::SectionDangerLead => "À modifier avec prudence.",
        M::CoreName => "Nom du core",
        M::CoreNameEmpty => "Le nom ne peut pas être vide",
        M::CoreNameTooLong => "Le nom ne peut pas dépasser {max} caractères",
        M::Domain => "Domaine ou IP publique",
        M::DomainTooLong => "Le domaine ne peut pas dépasser {max} caractères",
        M::DomainHint => {
            "C’est votre IP publique si vous avez configuré une redirection de port. Sinon, laissez tel quel."
        }
        M::Playit => "Activer l’intégration Playit",
        M::PlayitHint => "Ouvre les serveurs à Internet sans redirection de port.",
        M::ListPublicly => "Lister les serveurs publiquement",
        M::ListPubliclyHint => {
            "Toute personne qui peut joindre ce core voit les serveurs que vous rendez publics ci-dessous."
        }
        M::LocalClaim => {
            "Autoriser les applications de cet ordinateur à obtenir l’accès propriétaire"
        }
        M::LocalClaimHint => {
            "Lodestone Launcher peut devenir propriétaire en prouvant qu’il peut écrire dans le dossier de données de ce core. Tout ce qui peut y écrire peut déjà modifier chaque utilisateur et réglageu00a0: cela ne compte que sur un ordinateur partagé."
        }
        M::SafeMode => "Mode sécurisé",
        M::SafeModeHint => "Réserve l’accès administrateur de cette machine au propriétaire.",
        M::SafeModeOffTitle => "Désactiver le mode sécurisé ?",
        M::SafeModeOffBody => {
            "Voulez-vous vraiment désactiver le mode sécurisé ? Vous pourrez alors permettre à d’autres utilisateurs que vous d’exécuter des commandes potentiellement dangereuses. Assurez-vous de faire confiance à tous les utilisateurs à qui vous donnez ces autorisations."
        }
        M::TurnOff => "Désactiver",
        M::CodeExecution => "Exécution de code",
        M::CodeExecutionHint => {
            "Macros, serveurs atom, extensions, scripts de serveur et commandes Java personnalisées."
        }
        M::CodeExecutionHintJavaOnly => "Commandes Java personnalisées.",
        M::CodeExecutionLocked => {
            "Verrouillé par l’administrateur de ce core (LODESTONE_DISABLE_CODE_EXECUTION ou --disable-code-execution)."
        }
        M::CodeExecutionOffTitle => "Désactiver l’exécution de code ?",
        M::CodeExecutionOffBody => {
            "Les macros et serveurs atom en cours seront arrêtés. Les macros, serveurs atom, extensions, scripts de serveur et commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement."
        }
        M::CodeExecutionOffBodyJavaOnly => {
            "Les commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement."
        }
        M::NoScripting => {
            "Ce core a été compilé sans prise en charge des scripts (Deno) : les macros ne sont pas disponibles."
        }
        M::LanHttp => "Autoriser l’accès LAN en HTTP simple",
        M::LanHttpHint => {
            "Sans chiffrement. D’autres personnes sur votre réseau peuvent lire les mots de passe."
        }
        M::LanHttpConfirmTitle => "Autoriser le HTTP simple depuis d’autres appareils ?",
        M::LanHttpConfirmBody => {
            "Les appareils de votre réseau pourront se connecter en HTTP simple. Les mots de passe et jetons de session envoyés ainsi traversent le réseau sans chiffrement : n’importe qui d’autre sur le réseau peut les lire et prendre le contrôle de comptes sur ce core. N’autorisez cela que si un appareil ne peut pas utiliser https://."
        }
        M::LanHttpAllow => "Autoriser",
        M::LanHttpWarningOwner => {
            "D’autres appareils peuvent se connecter à ce core en HTTP simple : les mots de passe et jetons de session peuvent donc traverser votre réseau sans chiffrement. Désactivez « {setting} » ci-dessous et connectez-vous plutôt en https://."
        }
        M::LanHttpWarningOther => {
            "D’autres appareils peuvent se connecter à ce core en HTTP simple : les mots de passe et jetons de session peuvent donc traverser votre réseau sans chiffrement. Connectez-vous en https:// et demandez au propriétaire de désactiver l’accès en HTTP simple."
        }
        M::OpenPort => "Ouvrir un port",
        M::OpenPortHint => "Redirige un port sur votre routeur avec {upnp}.",
        M::OpenPortNoPermission => "Aucune autorisation",
        M::OpenPortButton => "Ouvrir un port",
        M::OpenPortPort => "Port à ouvrir",
        M::PortNotNumber => "Le port doit être un nombre",
        M::PortRequired => "Le port est requis",
        M::PortRange => "Le port doit être compris entre 1 et 65535",
        M::PortOpened => "Port {port} ouvert",
        M::InfoCoreVersion => "Version du core",
        M::InfoHostVersion => "Version de Lodestone Host",
        M::InfoOs => "Système d’exploitation",
        M::InfoArch => "Architecture",
        M::InfoCpu => "Processeur",
        M::InfoCpuCores => "Cœurs du processeur",
        M::InfoTotalRam => "Mémoire totale",
        M::InfoNoRam => "Pas de mémoire ? Comment faites-vous tourner Lodestone ?",
        M::InfoHttps => "Certificat HTTPS",
        M::InfoFingerprint => "Empreinte du certificat (SHA-256)",
        M::TlsSelfSigned => "Certificat auto-signé intégré",
        M::TlsUserProvided => "Votre propre certificat (tls/cert.pem)",
        M::TlsNone => "Indisponible, HTTP simple uniquement",
        M::ReadOnly => "Seul le propriétaire peut modifier ces paramètres.",
        M::ListingTitle => "Partage",
        M::ListingLead => "Qui peut trouver ce serveur sans se connecter.",
        M::ListedPublicly => "Listé publiquement",
        M::ListingPublic => "Public",
        M::ListingOwnerOnly => "Seul le propriétaire peut modifier ceci",
        M::FlagStatus => "État et joueurs",
        M::FlagAddress => "Adresse pour rejoindre",
        M::FlagClientPack => "Téléchargement des mods client",
        M::ShownText => {
            "Sans se connecter, toute personne qui peut accéder à ce core voit {list} de ce serveur."
        }
        M::ShownNameVersion => "le nom et la version",
        M::ShownStatus => "l’état et le nombre de joueurs",
        M::ShownAddress => "l’adresse",
        M::ShownClientPack => "les mods client",
        M::TurnOnFirst => "Activez d’abord « Lister les serveurs publiquement ».",
        M::HiddenWhileOff => {
            "{shown} Masqué tant que « Lister les serveurs publiquement » est désactivé."
        }
        M::NotShown => "Non visible sans se connecter.",
        M::NoJavaServers => "Aucun serveur Minecraft Java Edition pour l’instant.",
        M::UsersDescription => "Les comptes de ce core et leurs droits.",
        M::AllUsers => "Tous les utilisateurs",
        M::AllMembers => "Tous les membres",
        M::AllMembersCount => "Tous les membres ({count})",
        M::CreateUser => "Nouvel utilisateur",
        M::CreateUserTitle => "Créer un compte utilisateur",
        M::CreateUserLead => "Commence sans aucune autorisation.",
        M::LoadingUsers => "Chargement des utilisateurs",
        M::Permissions => "Autorisations",
        M::PermissionsLead => "Ce que {username} peut faire sur ce core.",
        M::UnsafePermissions => "Autorisations à risque",
        M::UnsafePermissionsLead => {
            "Désactivez le mode sécurisé pour les accorder à d’autres utilisateurs."
        }
        M::OwnerOnly => "Macros, extensions et serveurs atom",
        M::OwnerOnlyHint => {
            "Réservé au propriétaire. Les serveurs Docker utilisent les autorisations habituelles."
        }
        M::UsersOwnerOnly => "Seul le propriétaire gère les utilisateurs.",
        M::You => "Vous",
        M::Admin => "Admin",
        M::Owner => "Propriétaire",
        M::ChangePasswordMenu => "Changer le mot de passe",
        M::CantDeleteSelf => "Vous ne pouvez pas vous supprimer vous-même.",
        M::DeleteUserTitle => "Supprimer l’utilisateur",
        M::DeleteUserConfirm => {
            "Voulez-vous vraiment supprimer {name} ? Cette action est irréversible."
        }
        M::DeleteUser => "Supprimer l’utilisateur",
        M::NoServersYet => "Aucun serveur pour l’instant.",
        M::PermCreateInstance => "Créer des serveurs",
        M::PermCreateInstanceUser => "L’utilisateur peut créer de nouveaux serveurs.",
        M::PermCreateInstanceKey => "La clé peut créer de nouveaux serveurs.",
        M::PermDeleteInstance => "Supprimer des serveurs",
        M::PermDeleteInstanceUser => "L’utilisateur peut supprimer n’importe quel serveur.",
        M::PermDeleteInstanceKey => "La clé peut supprimer n’importe quel serveur.",
        M::PermReadGlobalFile => "Lire les fichiers globaux",
        M::PermReadGlobalFileUser => {
            "L’utilisateur peut lire n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute."
        }
        M::PermReadGlobalFileKey => {
            "La clé peut lire n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute."
        }
        M::PermViewInstance => "Voir les serveurs",
        M::PermViewInstanceUser => "L’utilisateur peut voir ces serveurs.",
        M::PermViewInstanceKey => "La clé peut voir ces serveurs.",
        M::PermStartInstance => "Démarrer les serveurs",
        M::PermStartInstanceUser => "L’utilisateur peut démarrer ces serveurs.",
        M::PermStartInstanceKey => "La clé peut démarrer ces serveurs.",
        M::PermStopInstance => "Arrêter les serveurs",
        M::PermStopInstanceUser => "L’utilisateur peut arrêter ces serveurs.",
        M::PermStopInstanceKey => "La clé peut arrêter ces serveurs.",
        M::PermConsole => "Accéder à la console des serveurs",
        M::PermConsoleUser => {
            "L’utilisateur peut lire la console de ces serveurs et y envoyer des commandes. Cela donne de fait un accès opérateur au serveur."
        }
        M::PermConsoleKey => {
            "La clé peut lire la console de ces serveurs et y envoyer des commandes. Cela donne de fait un accès opérateur au serveur."
        }
        M::PermInstanceSettings => "Accéder aux paramètres des serveurs",
        M::PermInstanceSettingsUser => {
            "L’utilisateur peut lire et modifier les paramètres de ces serveurs."
        }
        M::PermInstanceSettingsKey => {
            "La clé peut lire et modifier les paramètres de ces serveurs."
        }
        M::PermReadResource => "Lire les ressources des serveurs",
        M::PermReadResourceUser => {
            "L’utilisateur peut lire les ressources (mondes, mods, etc.) de ces serveurs."
        }
        M::PermReadResourceKey => {
            "La clé peut lire les ressources (mondes, mods, etc.) de ces serveurs."
        }
        M::PermReadFiles => "Lire les fichiers des serveurs",
        M::PermReadFilesUser => "L’utilisateur peut lire les fichiers de ces serveurs.",
        M::PermReadFilesKey => "La clé peut lire les fichiers de ces serveurs.",
        M::PermWriteResource => "Modifier les ressources des serveurs",
        M::PermWriteResourceUser => {
            "L’utilisateur peut modifier les ressources (mondes, mods, etc.) de ces serveurs."
        }
        M::PermWriteResourceKey => {
            "La clé peut modifier les ressources (mondes, mods, etc.) de ces serveurs."
        }
        M::PermWriteFiles => "Modifier les fichiers des serveurs",
        M::PermWriteFilesUser => "L’utilisateur peut modifier les fichiers de ces serveurs.",
        M::PermWriteFilesKey => "La clé peut modifier les fichiers de ces serveurs.",
        M::PermWriteGlobalFile => "Modifier les fichiers globaux",
        M::PermWriteGlobalFileUser => {
            "L’utilisateur peut modifier n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute."
        }
        M::PermWriteGlobalFileKey => {
            "La clé peut modifier n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute."
        }
        M::PermManagePermission => "Gérer les autorisations",
        M::PermManagePermissionUser => {
            "L’utilisateur peut gérer les autorisations des autres utilisateurs."
        }
        M::PermManagePermissionKey => {
            "La clé peut gérer les autorisations des autres utilisateurs."
        }
        M::Username => "Nom d’utilisateur",
        M::Password => "Mot de passe",
        M::OldPassword => "Ancien mot de passe",
        M::ConfirmPassword => "Confirmer le mot de passe",
        M::UsernameRequired => "Le nom d’utilisateur est requis",
        M::PasswordRequired => "Le mot de passe est requis",
        M::ConfirmRequired => "La confirmation du mot de passe est requise",
        M::PasswordsMustMatch => "Les mots de passe doivent être identiques",
        M::ChangePasswordButton => "Changer le mot de passe",
        M::CreateUserButton => "Créer l’utilisateur",
        M::Account => "Compte",
        M::ProfileUsername => "Nom d’utilisateur",
        M::ProfilePassword => "Mot de passe",
        M::ChangePassword => "Changer le mot de passe",
        M::Appearance => "Apparence",
        M::AppearanceLead => "Enregistré sur cet ordinateur, pas sur le core.",
        M::ColoredConsole => "Console en couleur",
        M::Language => "Langue",
        M::LanguageAuto => "Langue du système ({language})",
        M::Theme => "Thème",
        M::ThemeSystem => "Système",
        M::ThemeLight => "Clair",
        M::ThemeDark => "Sombre",
        M::ThemeSystemHint => "Suit le réglage clair ou sombre de votre ordinateur",
        M::ThemeLightHint => "Toujours clair",
        M::ThemeDarkHint => "Toujours sombre",
        M::Zoom => "Zoom",
        M::ZoomHint => "Aussi avec {modifier} + et {modifier} −.",
        M::ApiKeys => "Clés d’API",
        M::ApiKeysOwnLead => {
            "Permettent à la CLI, aux scripts et aux agents d’agir en votre nom sans votre mot de passe."
        }
        M::ApiKeysOwnEmpty => "Vous n’avez aucune clé d’API.",
        M::ApiKeysUserLead => {
            "Clés créées par {username}. En révoquer une coupe l’accès de ce qui l’utilise."
        }
        M::ApiKeysUserEmpty => "{username} n’a aucune clé d’API.",
        M::CreateApiKey => "Créer une clé d’API",
        M::ApiKeyCreated => "Clé d’API « {name} » créée",
        M::LoadingApiKeys => "Chargement des clés d’API",
        M::Revoke => "Révoquer",
        M::RevokeTitle => "Révoquer la clé d’API",
        M::RevokeBody => {
            "Révoquer « {name} » ({display}) ? Tout ce qui utilise cette clé perd l’accès immédiatement. Cette action est irréversible."
        }
        M::ApiKeyRevoked => "Clé d’API « {name} » révoquée",
        M::CreatedOn => "Créée le {date}",
        M::ExpiresOn => "Expire le {date}",
        M::ExpiredOn => "Expirée le {date}",
        M::ExpiresNever => "Expiration : jamais",
        M::LastUsed => "Dernière utilisation : {time}",
        M::LastUsedNever => "Dernière utilisation : jamais",
        M::ScopeFull => "Accès complet",
        M::ScopeLauncher => "Launcher : voir, démarrer et arrêter les serveurs",
        M::ScopeLauncherConsole => {
            "Launcher : voir, démarrer et arrêter les serveurs, lire et utiliser la console"
        }
        M::ScopeLauncherConsoleRead => {
            "Launcher : voir, démarrer et arrêter les serveurs, lire la console"
        }
        M::ScopeLauncherConsoleWrite => {
            "Launcher : voir, démarrer et arrêter les serveurs, envoyer des commandes à la console"
        }
        M::ScopeCustom => "Personnalisé : {servers}, {permissions}",
        M::PasswordSessionOnly => {
            "Les clés d’API ne peuvent être gérées que si vous vous connectez avec votre mot de passe, pas avec une clé d’API."
        }
        M::CreateApiKeyLead => {
            "Donnez à chaque clé une courte durée de validité et uniquement l’accès nécessaire."
        }
        M::ApiKeyName => "Nom",
        M::ApiKeyNameRequired => "Le nom est requis",
        M::ApiKeyNameTooLong => "Le nom doit comporter au plus {max} caractères",
        M::ExpiresIn => "Expire dans",
        M::Expiry30d => "30 jours",
        M::Expiry90d => "90 jours",
        M::Expiry1y => "1 an",
        M::ExpiryNever => "Jamais",
        M::NeverWarning => {
            "Une clé qui n’expire jamais fonctionne jusqu’à ce que vous la révoquiez. Choisissez une durée, sauf si vous en avez vraiment besoin."
        }
        M::Access => "Accès",
        M::AccessFull => "Accès complet (tout ce que vous pouvez faire)",
        M::AccessCustom => "Personnalisé",
        M::ChooseOne => "Choisissez au moins une autorisation.",
        M::CreateKey => "Créer la clé",
        M::OnlyYours => {
            "Une clé ne peut avoir que les autorisations que vous avez vous-même. Les autorisations et les serveurs que vous n’avez pas sont grisés."
        }
        M::NotYours => "Vous n’avez pas cette autorisation.",
        M::NoServers => "Vous ne voyez aucun serveur.",
        M::KeyUnsafe => "Paramètres à risque",
        M::KeyUnsafeSafeMode => {
            "Le mode sécurisé est activé : le core refuse donc ces autorisations aux clés à accès personnalisé tant que le mode sécurisé n’est pas désactivé dans les paramètres du core."
        }
        M::KeyOwnerOnly => "Macros, extensions et serveurs atom",
        M::KeyOwnerOnlyHint => {
            "Seules les clés à accès complet peuvent utiliser les macros, les extensions et les serveurs atom."
        }
        M::NewKeyTitle => "Clé d’API « {name} » créée",
        M::NewKeyCopyNow => "Copiez cette clé maintenant. Vous ne pourrez plus la voir.",
        M::NewKeyUseWithCli => "Utilisez-la avec la CLI Lodestone :",
        M::NewKeyStore => {
            "Conservez la clé comme un mot de passe. Toute personne qui la possède peut utiliser Lodestone avec ses accès jusqu’à son expiration ou sa révocation."
        }
        M::KeyCopied => "Clé d’API copiée dans le presse-papiers",
        M::CliCopied => "Exemple CLI copié dans le presse-papiers",
        M::MinecraftAccounts => "Comptes Minecraft",
        M::MinecraftOwnLead => "Liés depuis Lodestone Launcher.",
        M::MinecraftOwnEmpty => {
            "Aucun compte Minecraft n’est lié. Connectez-vous à ce core dans Lodestone Launcher pour en lier un."
        }
        M::MinecraftUserLead => {
            "Les comptes Minecraft que {username} a liés depuis Lodestone Launcher."
        }
        M::MinecraftUserEmpty => "{username} n’a lié aucun compte Minecraft.",
        M::MinecraftTooOld => "Ce core est trop ancien pour lier des comptes Minecraft.",
        M::MinecraftRemoveTitle => "Retirer le compte Minecraft",
        M::MinecraftRemoveBody => {
            "Retirer le lien vers {name} ? Il pourra être lié à nouveau depuis Lodestone Launcher."
        }
        M::MinecraftRemoved => "{name} retiré",
        M::Notifications => "Notifications",
        M::InProgress => "En cours",
        M::AllNotifications => "Toutes les notifications",
        M::LoadingNotifications => "Chargement des notifications",
        M::NoNotifications => "Aucune notification pour le moment !",
        M::EventStarting => "Le serveur {name} démarre !",
        M::EventRunning => "Le serveur {name} a démarré !",
        M::EventStopping => "Le serveur {name} s’arrête !",
        M::EventStopped => "Le serveur {name} s’est arrêté !",
        M::EventCrashed => "Le serveur {name} a rencontré une erreur !",
        M::WarningToast => {
            "Avertissement : un serveur a émis un avertissement. Cela ne devrait pas arriver, veuillez le signaler aux développeurs."
        }
        M::EventWarning => "Le serveur {name} a émis un avertissement",
        M::EventError => "Le serveur {name} a rencontré une erreur",
        M::PlayersJoined => "{players} a rejoint {name}",
        M::PlayersLeft => "{players} a quitté {name}",
        M::PlayersJoinedAndLeft => "{joined} a rejoint {name} et {left} a quitté {name}",
        M::PlayerMessage => "{player} a dit {message} sur {name}",
        M::EventApiKeyCreated => "Clé d’API « {name} » créée",
        M::EventApiKeyRevoked => "Clé d’API « {name} » révoquée",
        M::MacroStarted => "Macro {pid} démarrée sur {server}",
        M::MacroDetached => "Macro {pid} détachée sur {server}",
        M::MacroStopped => "Macro {pid} arrêtée sur {server} avec l’état {status}",
        M::PlayitStarted => "Agent Playitgg démarré !",
        M::PlayitLoading => "Chargement de l’agent Playitgg…",
        M::PlayitStopped => "Agent Playitgg arrêté !",
        M::LoadFailed => "Impossible de charger les notifications : {error}",
        M::CoreFallback => "Core",
        M::CoreFailedNotice => "Lodestone Core n’a pas pu démarrer : {error}",
        M::UpdatesTitle => "Mises à jour",
        M::UpdatesLead => {
            "Lodestone Host, sur cet ordinateur. Lodestone Core se met à jour avec lui."
        }
        M::ThisVersion => "Cette version",
        M::UpdateAutomatically => "Mettre à jour automatiquement",
        M::UpdateAutomaticallyHint => "Télécharge les nouveaux builds en arrière-plan.",
        M::BetaBuilds => "Builds bêta",
        M::BetaBuildsHint => {
            "Profitez des nouveautés avant tout le monde. Les builds bêta peuvent comporter des bugs. Si vous désactivez cette option, vous gardez votre build jusqu’à ce qu’un build stable plus récent sorte."
        }
        M::BuildNumber => "Build {build}",
        M::BuildNumberCommit => "Build {build} · {commit}",
        M::DevelopmentBuild => "Build de développement",
        M::UpdatesUnavailable => {
            "Lodestone Host ne se met pas encore à jour tout seul. Les nouvelles versions s’installent avec un nouveau téléchargement."
        }
        M::UpdateNotChecked => "Recherche de nouveaux builds toutes les quelques heures.",
        M::UpdateChecking => "Recherche d’un build plus récent…",
        M::UpToDate => "C’est le build le plus récent.",
        M::UpdateAvailable => "Le build {build} est disponible.",
        M::UpdateDownloading => "Téléchargement du build {build}… {percent}",
        M::UpdateReady => {
            "Le build {build} est installé. Redémarrez pour l’utiliser : vos serveurs s’arrêtent, puis redémarrent s’ils démarrent tout seuls."
        }
        M::UpdateFailed => "Impossible de mettre à jour : {error}",
        M::CheckForUpdates => "Vérifier",
        M::DownloadUpdate => "Télécharger",
        M::RestartToUpdate => "Redémarrer",
        M::BetaTitle => "Merci d’utiliser la bêta de Lodestone !",
        M::BetaBody => {
            "Merci d’utiliser la bêta de Lodestone ! Si vous rencontrez un problème, signalez-le-nous sur notre serveur Discord ou sur Github."
        }
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ScopeServers => ["{count} serveur", "{count} serveurs"],
        P::ScopePermissions => ["{count} autorisation", "{count} autorisations"],
    }
}
