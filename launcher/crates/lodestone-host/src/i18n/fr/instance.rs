use crate::i18n::{InstanceMsg as M, InstancePlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Automation => "Automatisation",
        M::AutomationLead => "Quand Lodestone démarre ce serveur de lui-même.",
        M::AutoStart => "Démarrage automatique",
        M::AutoStartHint => "Démarrer ce serveur au démarrage de Lodestone Core",
        M::RestartOnCrash => "Redémarrer après un plantage",
        M::RestartOnCrashHint => "Redémarrer ce serveur s’il plante",
        M::NotAvailable => "Indisponible pour le moment",
        M::NotAvailableHint => "Essayez de démarrer ce serveur au moins une fois",
        M::NoPermission => "Pas d’autorisation",
        M::SettingsFailed => "Impossible de charger les paramètres : {error}",
        M::DangerZone => "Zone dangereuse",
        M::DangerZoneLead => {
            "Ces paramètres peuvent causer des dommages irréversibles à votre serveur."
        }
        M::DeleteServer => "Supprimer le serveur",
        M::DeleteServerHint => "Supprime définitivement ce serveur et ses données",
        M::DeleteTitle => "Supprimer le serveur ?",
        M::DeleteConfirm => "Je comprends, supprimer ce serveur",
        M::DeleteCannotUndo => "Cette action est irréversible.",
        M::DeleteWarning => {
            "Ses paramètres et ses mondes seront définitivement supprimés. Sauvegardez vos données importantes avant de continuer."
        }
        M::DeleteBackupsToo => "Ses sauvegardes sont supprimées aussi.",
        M::Deleting => "Suppression de {name}…",
        M::CommandLineReadOnly => {
            "Lecture seule tant que l’exécution de code est désactivée sur ce core."
        }
        M::CommandLineOwnerOnly => "Seul le propriétaire peut modifier ce paramètre.",
        M::CannotBeEmpty => "Ne peut pas être vide",
        M::MustBeNumber => "Doit être un nombre",
        M::MustBeGreaterThan => "Doit être supérieur à {min}",
        M::MustBeLessThan => "Doit être inférieur à {max}",
        M::ShowPassword => "Afficher",
        M::HidePassword => "Masquer",
        M::Revert => "Rétablir",
        M::ResourcePack => "Pack de ressources",
        M::ResourcePackLead => {
            "Un pack que les joueurs sont invités à télécharger en rejoignant le serveur."
        }
        M::PackUrl => "URL",
        M::PackUrlHint => "Un lien direct vers le zip du pack. Laissez vide pour aucun pack.",
        M::PackSha1 => "SHA-1",
        M::PackSha1Hint => {
            "Permet aux joueurs de vérifier le téléchargement et de mettre le pack en cache."
        }
        M::PackSha1Error => "Le SHA-1 comporte 40 caractères hexadécimaux.",
        M::PackCompute => "Calculer",
        M::PackDownloaded => "Téléchargé",
        M::PackRequired => "Exiger le pack",
        M::PackRequiredHint => "Les joueurs qui le refusent sont déconnectés.",
        M::PackPrompt => "Message",
        M::PackPromptHint => {
            "Le message affiché quand le pack est proposé aux joueurs. Texte brut ou composant de texte JSON."
        }
        M::PackRemove => "Retirer le pack",
        M::PackSaved => "Pack de ressources enregistré",
        M::PackSavedRestart => {
            "Pack de ressources enregistré. Redémarrez le serveur pour l’utiliser."
        }
        M::PackUrlSpaces => "L’URL ne peut pas contenir d’espaces.",
        M::PackNotAUrl => "Ce n’est pas une URL.",
        M::PackUrlScheme => "L’URL doit commencer par http:// ou https://.",
        M::Eula => "CLUF de Minecraft",
        M::EulaLead => {
            "Un serveur Minecraft ne fonctionne qu’après votre accord avec le CLUF de Minecraft."
        }
        M::EulaAccepted => "Accepté",
        M::EulaAcceptedHint => "Vous avez accepté le CLUF de Minecraft pour ce serveur.",
        M::EulaNotAccepted => "Non accepté",
        M::EulaNotAcceptedHint => {
            "Ce serveur ne démarre pas tant que vous n’avez pas accepté le CLUF de Minecraft."
        }
        M::EulaRead => "Lire le CLUF",
        M::EulaAgree => "J’accepte le CLUF de Minecraft",
        M::EulaAcceptFailed => "Impossible d’accepter le CLUF de Minecraft : {error}",
        M::EulaNoPermission => "Il faut l’autorisation de démarrer ce serveur pour l’accepter.",
        M::EulaDialogTitle => "Accepter le CLUF de Minecraft",
        M::EulaDialogBody => {
            "Ce serveur ne démarre qu’après votre acceptation du CLUF de Minecraft. L’acceptez-vous ?"
        }
        M::EulaAcceptAndStart => "J’accepte, démarrer le serveur",
        M::EventLogsLead => {
            "Ce qui est arrivé à ce serveur, du plus récent au plus ancien. Le core garde les événements récents."
        }
        M::EventLogsEmpty => "Rien ne s’est encore passé",
        M::EventLogsFailed => "Impossible de charger les événements : {error}",
        M::EventState => "Le serveur est maintenant : {state}",
        M::EventJoined => "{names} a rejoint",
        M::EventLeft => "{names} est parti",
        M::EventBy => "par {user}",
        M::ModsUnsupported => {
            "Ce serveur ne prend pas en charge les mods ni les plugins. Les serveurs Fabric, Forge, Paper et Spigot, si."
        }
        M::ModsNoRead => "Vous n’avez pas l’autorisation de lire les fichiers de ce serveur.",
        M::ModsNoWrite => "Vous n’avez pas l’autorisation de modifier les fichiers de ce serveur",
        M::ModsInstalledView => "Installés",
        M::ModsBrowseView => "Parcourir",
        M::ModsRestartMod => "Redémarrez le serveur pour appliquer les modifications des mods",
        M::ModsRestartPlugin => {
            "Redémarrez le serveur pour appliquer les modifications des plugins"
        }
        M::ModsSearchMod => "Rechercher des mods sur Modrinth, ou coller un lien Modrinth",
        M::ModsSearchPlugin => "Rechercher des plugins sur Modrinth, ou coller un lien Modrinth",
        M::ModsSearchLabel => "Rechercher sur Modrinth",
        M::ModsOnlyCompatible => "Versions compatibles uniquement",
        M::ModsPlayersNeedToo => "Requis aussi pour les joueurs",
        M::ModsServerOnly => "Serveur uniquement",
        M::ModsPlayersNeed => "Requis pour les joueurs",
        M::ModsUnknown => "Inconnu",
        M::ModsUnknownModrinth => {
            "Modrinth n’indique pas si les joueurs en ont besoin. Consultez la page du mod, et cochez Pour les joueurs si c’est le cas."
        }
        M::ModsUnknownNotOnModrinth => {
            "Absent de Modrinth. Consultez la page du mod, et cochez Pour les joueurs si les joueurs en ont besoin."
        }
        M::ModsForPlayers => "Pour les joueurs",
        M::ModsForPlayersHint => {
            "Les joueurs reçoivent ce mod, et les mods dont il a besoin, en rejoignant. Cochez-le pour les mods dont les joueurs ont besoin sans que Lodestone puisse le savoir, comme ceux que vous avez téléversés."
        }
        M::ModsForPlayersAlready => {
            "Les joueurs reçoivent déjà ce mod : Modrinth ou le mod indique qu'ils en ont besoin."
        }
        M::ModsForPlayersOptional => {
            "Modrinth indique que les joueurs peuvent rejoindre sans ce mod, mais sur Fabric, les mods de ce genre les en empêchent souvent : les joueurs le reçoivent donc. Décochez-le pour le retirer."
        }
        M::ModsMarkedForPlayers => "Les joueurs le recevront à leur prochaine connexion",
        M::ModsUnmarkedForPlayers => "Les joueurs ne le recevront plus à leur connexion",
        M::ModsMarkFailed => "Impossible de le modifier : {error}",
        M::ModsRelease => "Stable",
        M::ModsBeta => "Bêta",
        M::ModsAlpha => "Alpha",
        M::ModsInstalledTag => "Installé",
        M::ModsSortBy => "Trier par",
        M::ModsSortRelevance => "Pertinence",
        M::ModsSortDownloads => "Téléchargements",
        M::ModsSortFollows => "Abonnés",
        M::ModsSortNewest => "Plus récents",
        M::ModsSortUpdated => "Mis à jour récemment",
        M::ModsOpening => "Ouverture du lien…",
        M::ModsLinkFailed => "Impossible d’ouvrir le lien : {error}",
        M::ModsOpenLink => "Ouvrir {slug}",
        M::ModsSearchFailed => "Échec de la recherche : {error}",
        M::ModsNoResults => "Aucun résultat.",
        M::ModsPage => "Page {page} sur {total}",
        M::ModsPreviousPage => "Page précédente",
        M::ModsNextPage => "Page suivante",
        M::ModsEnableFailed => "Impossible de l’activer : {error}",
        M::ModsDisableFailed => "Impossible de le désactiver : {error}",
        M::ModsUpdated => "Mis à jour",
        M::ModsUpdateFailed => "Impossible de le mettre à jour : {error}",
        M::ModsUpdateTo => "Passer à {version}",
        M::ModsViewOnModrinth => "Voir sur Modrinth",
        M::ModsDeleteLabel => "Supprimer {name}",
        M::ModsUploaded => "Importé",
        M::ModsUploadFailed => "Échec de l’importation : {error}",
        M::ModsOnlyJars => "Seuls les fichiers .jar peuvent être importés. Non importés :",
        M::ModsUpdatesInstalled => "Mises à jour installées",
        M::ModsDeleted => "Supprimé",
        M::ModsDeleteFailed => "Échec de la suppression : {error}",
        M::ModsDeleteTitleMod => "Supprimer le mod ?",
        M::ModsDeleteTitlePlugin => "Supprimer le plugin ?",
        M::ModsDeleteBody => "Son fichier sera supprimé du serveur. Cette action est irréversible.",
        M::ModsOverwriteTitle => "Remplacer les fichiers existants ?",
        M::ModsOverwrite => "Remplacer",
        M::ModsOverwriteBody => "{message} Le remplacer par le fichier importé ?",
        M::ModsOverwriteDefault => "Un fichier du même nom est déjà installé.",
        M::ModsUpdateAll => "Tout mettre à jour",
        M::ModsCheckUpdates => "Rechercher des mises à jour",
        M::ModsUpload => "Importer",
        M::ModsUploadPrompt => "Importer",
        M::ModsLoadFailedMod => "Impossible de charger les mods : {error}",
        M::ModsLoadFailedPlugin => "Impossible de charger les plugins : {error}",
        M::ModsEmptyMod => {
            "Aucun mod installé pour l’instant. Parcourez Modrinth ou importez un fichier .jar."
        }
        M::ModsEmptyPlugin => {
            "Aucun plugin installé pour l’instant. Parcourez Modrinth ou importez un fichier .jar."
        }
        M::ModsPlainFabric => "Les joueurs peuvent rejoindre avec Fabric seul, dans ces versions.",
        M::ModsDownloadPack => "Télécharger le modpack client (.mrpack)",
        M::ModsPackShowFewer => "Afficher moins",
        M::ModsDownloadPackFailed => "Impossible de télécharger le modpack client : {error}",
        M::ModsPackSaved => "{name} enregistré",
        M::ModsSavePack => "Enregistrer",
        M::ModsImportHint => {
            "Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions."
        }
        M::ModpackFrom => "Ce serveur utilise ce modpack.",
        M::ModpackCheck => "Rechercher une mise à jour",
        M::ModpackCheckFailed => "Impossible de rechercher une mise à jour : {error}",
        M::ModpackUpToDate => "C’est la version la plus récente.",
        M::ModpackUpdateTo => "Mettre à jour vers {version}",
        M::ModpackFromFile => "Mettre à jour depuis un fichier",
        M::ModpackFilePrompt => "Mettre à jour",
        M::ModpackNotMrpack => "Ce n’est pas un fichier .mrpack",
        M::ModpackPlanFailed => "Impossible de préparer la mise à jour : {error}",
        M::ModpackDialogTitle => "Mettre à jour le modpack ?",
        M::ModpackDialogRestoreIntro => {
            "Les mods et les configurations reviennent à leur état après ce changement."
        }
        M::ModpackDialogReinstall => "Le serveur passe à ce jeu et ce chargeur, et le réinstalle.",
        M::ModpackDialogKept => "Restent tels que vous les avez modifiés",
        M::ModpackDialogNothing => "Aucun fichier ne change.",
        M::ModpackDialogStop => {
            "Arrêtez d’abord le serveur : il ne peut pas être mis à jour pendant qu’il tourne."
        }
        M::ModpackDialogUnsupported => "Cette version ne peut pas être utilisée : {reason}",
        M::ModpackUpdate => "Mettre à jour",
        M::ModpackUpdated => "Mis à jour vers {version}",
        M::ModpackUpdateFailed => "Impossible de mettre à jour le modpack : {error}",
        M::ModpackUnknownVersion => "version inconnue",
        M::ModpackWhatsNew => "Nouveautés",
        M::ModpackHistory => "Historique",
        M::ModpackHistoryFailed => "Impossible de charger l’historique du modpack : {error}",
        M::ModpackHistoryEmpty => {
            "Rien n’est encore enregistré. Les prochaines modifications pourront être annulées."
        }
        M::ModpackCurrent => "Actuelle",
        M::ModpackRestore => "Restaurer",
        M::ModpackRestoreMissing => "Lodestone n’a plus certains fichiers de cette version.",
        M::ModpackRestoreTitle => "Restaurer cette version ?",
        M::ModpackRestored => "Restauré",
        M::ModpackRestoreFailed => "Impossible de restaurer cette version : {error}",
        M::ModpackSinceTooltip => "Inchangé depuis : {change}",
        M::ModsProjectFailed => "Impossible de charger le projet : {error}",
        M::ModsSource => "Code source",
        M::ModsIssues => "Tickets",
        M::ModsWiki => "Wiki",
        M::ModsServerSide => "Côté serveur",
        M::ModsSideRequired => "Requis",
        M::ModsSideOptional => "Facultatif",
        M::ModsSideUnsupported => "Non pris en charge",
        M::ModsSideUnknown => "Inconnu",
        M::ModsCreateFromModpack => "Créer un serveur à partir de ce modpack",
        M::ModsModpackNoPermission => {
            "Ceci est un modpack. Vous n’avez pas l’autorisation de créer des serveurs."
        }
        M::ModsNoServerSupport => "D’après ce projet, il ne fonctionne pas sur les serveurs.",
        M::ModsWrongTypeMod => {
            "Ce projet est de type {type}. Lodestone n’installe ici que des mods."
        }
        M::ModsWrongTypePlugin => {
            "Ce projet est de type {type}. Lodestone n’installe ici que des plugins."
        }
        M::ModsDescription => "Description",
        M::ModsRestOnModrinth => "La suite de la description est sur Modrinth.",
        M::ModsVersions => "Versions",
        M::ModsWithDependencies => "Installer aussi les dépendances requises",
        M::ModsShowAll => "Afficher toutes les versions",
        M::ModsVersionsFailed => "Impossible de charger les versions : {error}",
        M::ModsNoVersions => "Aucune version pour le jeu et le chargeur de ce serveur.",
        M::ModsNoVersionsShowAll => {
            "Aucune version pour le jeu et le chargeur de ce serveur. Activez « Afficher toutes les versions » pour voir les autres."
        }
        M::ModsNotFor => "Pas pour ce serveur",
        M::ModsInstalledVersion => "Installé",
        M::ModsInstallFailed => "Impossible de l’installer : {error}",
        M::ModsIncompatibleTitle => "Installer une version incompatible ?",
        M::ModsInstallAnyway => "Installer quand même",
        M::ModsIncompatibleBody => "Il risque de faire planter le serveur.",
        M::FriendsTitle => "Amis",
        M::FriendsLead => {
            "Les amis ajoutés dans Lodestone Launcher voient les serveurs que vous partagez avec eux et les rejoignent sans lien. Rien n’est partagé tant que vous ne cochez pas un ami."
        }
        M::FriendsAddInLauncher => "Ajoutez des amis dans Lodestone Launcher",
        M::FriendsAddInLauncherHint => {
            "Votre liste d’amis est gérée par Lodestone Launcher sur cet ordinateur. Une fois que vous y avez des amis, vous pouvez partager ce serveur avec eux ici."
        }
        M::FriendsLoading => "Chargement de vos amis…",
        M::FriendsShareWith => "Partager ce serveur",
        M::FriendsNotVerified => "Non vérifié",
        M::FriendsMinecraft => {
            "Ajoutés à la liste blanche quand vous partagez, si le serveur l’a activée."
        }
        M::FriendsWhitelisted => "Ajouté à la liste blanche du serveur",
        M::FriendsWhitelistFailed => {
            "Impossible de les ajouter à la liste blanche du serveur : {error}"
        }
        M::FriendsConnected => "Connecté",
        M::FriendsSyncFailed => {
            "Impossible de synchroniser votre liste d’amis avec ce core : {error}"
        }
        M::FriendsOpenFailed => "Impossible d’ouvrir votre liste d’amis : {error}",
        M::FriendsPublishFailed => "Impossible de prévenir {name} de ce core : {error}",
        M::FriendsPublishFailedAll => "Impossible de prévenir vos amis de ce core : {error}",
        M::FriendsRetry => "Réessayer",
        M::FriendsRetrying => "Lodestone Host réessaie chaque minute.",
        M::FriendsSynced => "Liste d’amis synchronisée",
        M::FriendsRetired => "Votre identité d’ami a été retirée",
        M::FriendsRetiredHint => {
            "Ce core ne partage plus avec personne. Pour recommencer, choisissez un nouveau nom dans la section Amis de Lodestone Launcher, puis ajoutez de nouveau vos amis."
        }
        M::FriendsOff => "Le partage avec des amis est désactivé dans cette copie de test",
        M::FriendsOffHint => {
            "Son core ou sa liste d’amis est dans un dossier de test, et l’autre non : vos vrais amis n’atteignent donc jamais un core de test. Pour tester le partage, définissez LODESTONE_PATH et LODESTONE_LAUNCHER_DATA_DIR sur des dossiers de test."
        }
        M::FriendsConfirmUnverified => {
            "Vous n’avez pas encore comparé les codes de sécurité avec cet ami, vous ne pouvez donc pas être sûr que c’est bien lui : quelqu’un d’autre a pu répondre à votre lien d’ami. Comparez d’abord les codes dans Lodestone Launcher, ou partagez quand même."
        }
        M::FriendsShareAnyway => "Partager quand même",
        M::FriendsCertificate => "Certificat d’ami de ce core",
        M::FriendsCertificateHint => {
            "Vos amis reconnaissent ce core grâce à lui. Retirez-le si les données de cet ordinateur ont pu être copiées : vos amis l’oublient, et le core en reçoit un nouveau qu’ils trouvent à la place."
        }
        M::FriendsCertificateRetire => "Retirer le certificat…",
        M::FriendsCertificateRetireBody => {
            "Vos amis oublient le certificat actuel de ce core et ne font plus confiance aux mods de ses serveurs. Le core reçoit un nouveau certificat, sous lequel vos amis le trouvent à leur prochaine vérification. Toutes les personnes connectées sont déconnectées."
        }
        M::FriendsCertificateRetireConfirm => "Retirer et en obtenir un nouveau",
        M::FriendsCertificateRetired => "Le certificat d’ami de ce core a été retiré",
        M::FriendsCertificateRetiredHint => {
            "Vos amis ne voient plus ce core. Donnez-lui un nouveau certificat pour partager de nouveau depuis cet ordinateur."
        }
        M::FriendsCertificateNew => "Obtenir un nouveau certificat",
        M::FriendsCertificateManual => {
            "Vos amis ne voient plus ce core. Il ne peut pas créer lui-même un nouveau certificat : quittez Lodestone Host, supprimez ce fichier, puis rouvrez-le."
        }
        M::FriendsCancel => "Annuler",
        M::MemoryKeywords => "mémoire RAM",
        M::JvmArgsKeywords => "JVM Java arguments options",
        M::WhitelistKeywords => "liste blanche joueurs autorisés",
        M::MotdKeywords => "MOTD message du jour description du serveur",
        M::AutoStartKeywords => "démarrage lancement automatique",
        M::ModsPluginsLabel => "Plugins",
        M::ModpackUpdateAvailable => "Mise à jour disponible",
        M::ModpackReplacedKey => "Remplacés",
        M::ModpackRecorded => "Enregistré",
        M::ModsThisVersion => "Cette version",
        M::ModsThisServer => "Ce serveur",
        M::FriendsMinecraftKey => "Comptes Minecraft",
        M::FriendsShared => "Partagé",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModsPackTitle => [
            "Les joueurs ont besoin de {count} mod pour se connecter",
            "Les joueurs ont besoin de {count} mods pour se connecter",
        ],
        P::ModsImportHintJars => [
            "Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions. {count} mod n’est pas sur Modrinth, son fichier est donc inclus dans le pack.",
            "Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions. {count} mods ne sont pas sur Modrinth, leurs fichiers sont donc inclus dans le pack.",
        ],
        P::ModsPackShowAll => ["Afficher {count} mod", "Afficher les {count} mods"],
        P::ModsShowMore => [
            "Afficher plus ({count} restante)",
            "Afficher plus ({count} restantes)",
        ],
        P::ModpackAdded => ["{count} fichier ajouté", "{count} fichiers ajoutés"],
        P::ModpackReplaced => ["{count} fichier remplacé", "{count} fichiers remplacés"],
        P::ModpackRemoved => ["{count} fichier supprimé", "{count} fichiers supprimés"],
        P::ModpackMoreChanges => ["et {count} autre", "et {count} autres"],
        P::FriendsConnectedCount => ["{count} ami connecté", "{count} amis connectés"],
        P::ModsNeededBy => ["Requis par {count} mod", "Requis par {count} mods"],
        P::ModsResultInstalled => ["{count} mod installé", "{count} mods installés"],
        P::ModsResultRemoved => ["{count} fichier supprimé", "{count} fichiers supprimés"],
        P::ModsResultSkipped => ["{count} projet ignoré", "{count} projets ignorés"],
        P::ModsResultPlayersNeed => [
            "Les joueurs en auront aussi besoin ({count})",
            "Les joueurs en auront aussi besoin ({count})",
        ],
        P::ModsSomeUpdatesFailed => [
            "{count} mise à jour a échoué",
            "{count} mises à jour ont échoué",
        ],
        P::ModsUploadedCount => ["{count} mod importé", "{count} mods importés"],
    }
}
