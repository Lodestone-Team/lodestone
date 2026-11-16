use crate::i18n::{InstancesMsg as M, InstancesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::CopyAddress => "Copier l’adresse",
        M::CopiedText => "Copié",
        M::SettingUpPercent => "Configuration… {percent}",
        M::StepSelectGame => "Choisir un jeu",
        M::StepBasicSettings => "Paramètres de base",
        M::StepServerSettings => "Paramètres du serveur",
        M::StepAutoSettings => "Automatisation",
        M::StepSelectModpack => "Choisir un modpack",
        M::StepSelectVersion => "Choisir une version",
        M::BasicSettingsDescription => "Les paramètres de base de votre serveur.",
        M::ServerSettingsDescription => "Configurez votre serveur.",
        M::AutoSettingsDescription => "Configurez automatiquement votre serveur.",
        M::CheckFields => "Vérifiez que les champs sont correctement remplis",
        M::CreateServer => "Créer le serveur",
        M::CreateFailed => "Erreur lors de la création du serveur : {error}",
        M::LoadingForm => "Chargement du formulaire de création",
        M::FormFailed => "Impossible de charger le formulaire : {error}",
        M::GamesFailed => "Impossible de charger les jeux : {error}",
        M::UnknownSetting => "Ce paramètre nécessite une version plus récente de Lodestone Host.",
        M::EulaAgree => "J’accepte le {link}",
        M::EulaLink => "CLUF de Minecraft",
        M::EulaRequired => {
            "Un serveur Minecraft ne fonctionne qu’après votre accord avec le CLUF de Minecraft."
        }
        M::FieldName => "Nom",
        M::FieldNameDescription => "Le nom du serveur",
        M::FieldNameDefault => "Mon serveur",
        M::FieldDescription => "Description",
        M::FieldDescriptionDescription => "La description du serveur",
        M::FieldPort => "Port",
        M::FieldPortDescription => "Le port sur lequel exécuter le serveur",
        M::FieldMinRam => "Mémoire minimale",
        M::FieldMinRamDescription => "La quantité minimale de mémoire à allouer au serveur, en Mo",
        M::FieldMaxRam => "Mémoire maximale",
        M::FieldMaxRamDescription => "La quantité maximale de mémoire à allouer au serveur, en Mo",
        M::FieldAutoStart => "Démarrage automatique",
        M::FieldAutoStartDescription => {
            "Le serveur démarrera automatiquement au lancement de l’application"
        }
        M::FieldRestartOnCrash => "Redémarrer en cas de plantage",
        M::FieldRestartOnCrashDescription => "Le serveur redémarrera automatiquement s’il plante",
        M::Optional => "Facultatif",
        M::ValidationRequired => "Le champ {name} est obligatoire",
        M::ValidationNumber => "Le champ {name} doit être un nombre",
        M::ValidationInteger => "Le champ {name} doit être un entier",
        M::ValidationPositive => "Le champ {name} doit être positif",
        M::ValidationMin => "Le champ {name} doit être supérieur ou égal à {min}",
        M::ValidationNameCharacters => {
            "Le nom ne peut contenir aucun de ces caractères : / \\ : * ? \" < > |"
        }
        M::ValidationMax => "Le champ {name} doit être inférieur ou égal à {max}",
        M::ValidationOneOf => "Le champ {name} doit correspondre à l’une des options proposées",
        M::MaxRamBelowMin => "La mémoire maximale doit être au moins égale à la mémoire minimale",
        M::NothingFound => "Aucun résultat.",
        M::CommandLineReadOnly => {
            "Lecture seule tant que l’exécution de code est désactivée sur ce core."
        }
        M::CommandLineOwnerOnly => "Seul le propriétaire peut modifier ce paramètre.",
        M::GameTypeTitle => "Choisissez un jeu",
        M::GameTypeSubtitle => "À quoi servira votre serveur ?",
        M::GameTypeNew => "Nouveau",
        M::GameTypeModpack => "Modpack Modrinth",
        M::GameTypeModpackDescription => {
            "Créer un serveur Fabric, Forge ou NeoForge à partir d’un modpack Modrinth"
        }
        M::GameTypeImport => "Serveur existant",
        M::GameTypeImportDescription => {
            "Importer un serveur Minecraft que vous avez déjà, avec son monde, ses paramètres, ses mods et ses plugins"
        }
        M::StepChooseFolder => "Choisir le dossier",
        M::ImportFolderTitle => "Choisissez le dossier du serveur",
        M::ImportFolderSubtitle => {
            "Le dossier qui contient server.properties et le monde. Lodestone le copie et laisse l’original tel quel."
        }
        M::ImportFolderPlaceholder => "Chemin du dossier",
        M::ImportBrowse => "Parcourir…",
        M::ImportChooseFolder => "Choisissez d’abord le dossier du serveur",
        M::ImportStopFirst => {
            "Si le serveur est en marche, arrêtez-le d’abord pour que son monde soit copié en entier."
        }
        M::ImportReading => "Lecture du dossier",
        M::ImportReadFailed => "Impossible de lire le dossier : {error}",
        M::ImportSettingsSubtitle => "Vérifiez ce que Lodestone a trouvé, puis importez.",
        M::ImportOtherSoftware => {
            "Ce serveur utilisait {other}. Lodestone le fait tourner avec {software}, qui utilise les mêmes plugins."
        }
        M::ImportNotOffered => {
            "Ce serveur utilisait {other}, que Lodestone ne propose pas. Choisissez le logiciel avec lequel le faire tourner."
        }
        M::ImportSoftwareUnknown => {
            "Lodestone n’a pas pu déterminer le logiciel de ce serveur. Choisissez-le ci-dessous."
        }
        M::FieldSoftware => "Logiciel serveur",
        M::FieldSoftwareDescription => {
            "Ce qui fait tourner le serveur. Lodestone l’installe à nouveau."
        }
        M::FieldVersion => "Version de Minecraft",
        M::FieldVersionDescription => "La dernière version utilisée par le serveur",
        M::ImportLoadingVersions => "Chargement des versions",
        M::ImportVersionsFailed => "Impossible de charger les versions : {error}",
        M::ImportNoVersions => "le core n’a envoyé aucune version",
        M::ImportVersionUnavailable => {
            "Lodestone n’a pas de serveur {software} pour Minecraft {version}. Choisissez une autre version."
        }
        M::ImportChooseSoftware => "Choisissez le logiciel serveur",
        M::ImportServer => "Importer le serveur",
        M::ImportFailed => "Impossible d’importer le serveur : {error}",
        M::GameTypeImage => "Depuis un fichier",
        M::GameTypeImageDescription => {
            "Créer un serveur à partir d’un fichier .lodeimg exporté par un autre Host ou un Launcher"
        }
        M::StepChooseFile => "Choisir un fichier",
        M::StepImageSummary => "Son contenu",
        M::ImageFileTitle => "Choisir un fichier .lodeimg",
        M::ImageFileSubtitle => {
            "Une sauvegarde exportée depuis Lodestone Host, ou un instantané exporté depuis Lodestone Launcher. Rien n’est créé avant que vous ne créiez le serveur."
        }
        M::ImageBrowse => "Choisir un fichier…",
        M::ImageUploading => "Lecture du fichier…",
        M::ImageUploadFailed => "Impossible d’ouvrir le fichier : {error}",
        M::ImageSummaryTitle => "Contenu du fichier",
        M::ImageSummarySubtitle => "Vérifiez le contenu du fichier, puis créez le serveur.",
        M::ImageKeepOperators => "Garder les opérateurs du fichier : {names}",
        M::ImageKeepOperatorsHint => {
            "Les opérateurs peuvent exécuter n’importe quelle commande sur le serveur."
        }
        M::ImageCreate => "Créer le serveur",
        M::ImageCreateFailed => "Impossible de créer le serveur : {error}",
        M::CreateFailedLater => "Le serveur n’a pas pu être créé",
        M::ImageNotLodeimg => "{name} n’est pas un fichier .lodeimg",
        M::GameVanillaDescription => "Serveur Minecraft vanilla standard de Mojang.",
        M::GameForgeDescription => {
            "Framework de modding pour installer des mods et personnaliser votre expérience Minecraft."
        }
        M::GameNeoForgeDescription => {
            "Fork communautaire de Forge, avec ses propres mods, pour Minecraft 1.20.2 et versions ultérieures."
        }
        M::GameFabricDescription => "Chaîne d’outils de modding légère pour Minecraft.",
        M::GamePaperDescription => {
            "Fork de Spigot hautes performances qui vise à corriger les incohérences de gameplay et de mécaniques."
        }
        M::GameSpigotDescription => {
            "Logiciel de serveur Minecraft modifié, compatible avec les plugins, aux performances et options de personnalisation améliorées."
        }
        M::GameBedrockDescription => "Serveur Minecraft Bedrock Edition.",
        M::GameOtherDescription => "Variante de Minecraft inconnue : {name}",
        M::GameGenericDescription => "Jeu inconnu : {name}",
        M::ModpackSourceTitle => "Choisissez un modpack",
        M::ModpackSourceDescription => {
            "Recherchez sur Modrinth, collez un lien Modrinth ou importez un fichier .mrpack. Les modpacks Fabric, Forge et NeoForge sont pris en charge."
        }
        M::ModpackSearchPlaceholder => "Rechercher des modpacks ou coller un lien Modrinth",
        M::ModpackUpload => "Importer un .mrpack",
        M::ModpackVersionTitle => "Choisissez une version",
        M::ModpackVersionUploaded => "Le modpack importé.",
        M::ModpackVersionDescription => "Choisissez la version du modpack à installer.",
        M::ModpackDownloading => "Téléchargement du fichier du modpack pour le lire…",
        M::ModpackDownloadingSize => "Téléchargement du fichier du modpack ({size}) pour le lire…",
        M::ModpackVersionHint => {
            "Choisissez une version pour voir son contenu. Lodestone télécharge le fichier du modpack pour le lire ; rien n’est installé tant que vous n’avez pas cliqué sur Créer le serveur."
        }
        M::ModpackSettingsTitle => "Paramètres du serveur",
        M::ModpackLoading => "Chargement du modpack",
        M::ModpackLoadingVersions => "Chargement des versions",
        M::ModpackVersionsFailed => "Impossible de charger les versions : {error}",
        M::ModpackNoVersions => "Ce modpack n’a aucune version.",
        M::ModpackSelect => "Choisir",
        M::ModpackSelected => "Choisie",
        M::ModpackSummaryModpack => "Modpack",
        M::ModpackSummaryLoader => "Chargeur",
        M::ModpackSummaryServerFiles => "Fichiers du serveur",
        M::ModpackSummaryServerFilesValue => "{count} ({size} à télécharger)",
        M::ModpackUnsupported => "Ce modpack ne peut pas être utilisé : {reason}",
        M::ModpackInspectFailed => "Impossible de lire le modpack : {error}",
        M::ModpackNotAModpack => "Ce projet est de type {type}, pas un modpack",
        M::ModpackChooseMrpack => "Choisissez un fichier .mrpack",
        M::ModpackNoUploadId => "Le core n’a renvoyé aucun identifiant d’importation",
        M::ModpackUploadFailed => "Impossible d’importer le modpack : {error}",
        M::ModpackUploading => "Envoi de {name}…",
        M::SortLabel => "Trier par",
        M::SortRelevance => "Pertinence",
        M::SortDownloads => "Téléchargements",
        M::SortFollows => "Abonnés",
        M::SortNewest => "Plus récents",
        M::SortUpdated => "Mis à jour récemment",
        M::SearchOpening => "Ouverture du lien…",
        M::SearchLinkFailed => "Impossible d’ouvrir le lien : {error}",
        M::SearchOpenLink => "Ouvrir {slug}",
        M::SearchFailed => "Échec de la recherche : {error}",
        M::SearchLoading => "Chargement des résultats",
        M::SearchNoResults => "Aucun résultat.",
        M::SearchPage => "Page {page} sur {total}",
        M::PreviousPage => "Page précédente",
        M::NextPage => "Page suivante",
        M::VersionRelease => "stable",
        M::VersionBeta => "bêta",
        M::VersionAlpha => "alpha",
        M::NotFound => "Serveur introuvable",
        M::NoName => "Sans nom",
        M::NoDescription => "Aucune description",
        M::EditName => "Renommer",
        M::EditDescription => "Modifier la description",
        M::NameEmpty => "Saisissez un nom.",
        M::DetailsTitle => "Détails",
        M::DetailsName => "Nom",
        M::DetailsDescription => "Description",
        M::DetailsUuid => "UUID",
        M::DetailsGame => "Jeu",
        M::DetailsMinecraftVersion => "Version de Minecraft",
        M::DetailsVersion => "Version",
        M::DetailsLoader => "Chargeur",
        M::DetailsPort => "Port",
        M::DetailsState => "État",
        M::DetailsPlayers => "Joueurs",
        M::DetailsCreated => "Créé le",
        M::DetailsAutoStart => "Démarrage automatique",
        M::DetailsRestartOnCrash => "Redémarrer après un plantage",
        M::DetailsFolder => "Dossier",
        M::PerformanceTitle => "Surveillance du serveur",
        M::PerformanceLead => "Utilisation de votre Lodestone Core par l’instance",
        M::CpuLoad => "Charge CPU",
        M::MemoryUsed => "Mémoire utilisée",
        M::LoadingGraph => "Chargement du graphique",
        M::CpuUsage => "Utilisation du CPU",
        M::MemoryUsage => "Utilisation de la mémoire",
        M::UnitPercent => " %",
        M::UnitGiB => " Gio",
        M::GraphSeconds => "{seconds} s",
        M::GraphSecondsAgo => "il y a {seconds} s",
        M::PlayersTitle => "Liste des joueurs",
        M::PlayersLead => "Tous les joueurs actuellement en ligne",
        M::PlayersSortByName => "NOM",
        M::PlayersLinkedUser => "L’utilisateur Lodestone qui a associé ce compte Minecraft",
        M::PlayersNone => "Aucun joueur en ligne",
        M::ShareTitle => "Partager",
        M::ShareLead => {
            "Choisissez des amis dans votre liste d’amis, ou envoyez un lien à n’importe qui"
        }
        M::ShareFriendsHeading => "Vos amis",
        M::ShareFriendsNote => {
            "Pas besoin de lien. Ils voient ce serveur dans Lodestone Launcher, sous votre nom, tant qu’il tourne."
        }
        M::ShareFriendsNone => {
            "Ajoutez des amis dans Lodestone Launcher (page Lodestone Host, Amis) pour partager avec eux sans lien."
        }
        M::ShareLinkHeading => "Ou envoyez un lien d’invitation",
        M::ShareOff => {
            "Envoyez un lien à qui vous voulez. Ces personnes l'ouvrent dans Lodestone Launcher, et leur jeu se connecte directement à cet ordinateur. Aucune redirection de port nécessaire."
        }
        M::ShareStart => "Partager avec un lien",
        M::ShareStarting => "Démarrage…",
        M::ShareSharing => "Partagé",
        M::ShareReconnecting => "Reconnexion…",
        M::ShareStableLink => "Le lien reste valable après un redémarrage.",
        M::ShareLinkLabel => "Lien d'invitation",
        M::ShareCopy => "Copier le lien",
        M::ShareCopied => "Lien d'invitation copié",
        M::SharePreparing => "Création du lien…",
        M::ShareHowToJoin => {
            "Toute personne ayant le lien peut rejoindre ce serveur pendant qu'il tourne. Les personnes sans Lodestone Launcher reçoivent un lien de téléchargement."
        }
        M::ShareHardNetwork => {
            "Le routeur de ce réseau rend les connexions directes difficiles. Certaines personnes ne pourront peut-être pas rejoindre."
        }
        M::ShareSameComputer => "Cet ordinateur",
        M::ShareLocalNetwork => "Réseau local",
        M::ShareIpv6 => "IPv6",
        M::ShareInternet => "Internet",
        M::ShareStop => "Arrêter le partage",
        M::ShareStopTitle => "Arrêter le partage ?",
        M::ShareStopBody => {
            "Les personnes qui jouent via le lien sont déconnectées et le lien ne fonctionne plus. Partager à nouveau crée un nouveau lien."
        }
        M::ShareNewLink => "Nouveau lien",
        M::ShareNewLinkTitle => "Créer un nouveau lien ?",
        M::ShareNewLinkBody => {
            "Le lien actuel ne fonctionne plus et les personnes qui jouent via ce lien sont déconnectées. Envoyez le nouveau lien aux personnes qui doivent encore pouvoir rejoindre. Les amis avec qui vous partagez depuis votre liste d'amis ne sont pas concernés."
        }
        M::ShareNewLinkConfirm => "Créer un nouveau lien",
        M::ShareNewLinkMade => "Nouveau lien d'invitation créé",
        M::ShareRemovedFriendsNote => {
            "Vous avez retiré ces amis. S’ils ont ce lien d’invitation, ils peuvent encore rejoindre jusqu’à ce que vous en créiez un nouveau."
        }
        M::ShareRemovedFriendsDismiss => "Ignorer",
        M::ShareRemovedFriendsToast => {
            "{names} peuvent encore rejoindre {servers} s'ils ont le lien d'invitation. Créez de nouveaux liens pour les en empêcher."
        }
        M::ShareNewLinks => "Créer de nouveaux liens",
        M::ShareNewLinksMade => {
            "Nouveaux liens d'invitation créés. Envoyez-les aux personnes qui doivent encore pouvoir rejoindre."
        }
        M::ShareNewLinkFailed => "Impossible de créer un nouveau lien pour {name} : {error}",
        M::ShareShort => "Partager",
        M::ShareShortHint => "Partager ce serveur",
        M::DetailsMinRam => "RAM min.",
        M::DetailsMaxRam => "RAM max.",
        M::ShareConnected => "Personnes connectées",
        M::ShareJoins => "Connexions jusqu’ici",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModpackShowMore => [
            "Afficher plus ({count} restante)",
            "Afficher plus ({count} restantes)",
        ],
        P::ImportFiles => ["{count} fichier", "{count} fichiers"],
    }
}
