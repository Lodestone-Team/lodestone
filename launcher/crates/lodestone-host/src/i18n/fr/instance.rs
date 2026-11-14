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
        M::DeleteTitle => "Supprimer « {name} »",
        M::DeleteConfirm => "Je comprends, supprimer ce serveur",
        M::DeleteCannotUndo => "Cette action est irréversible.",
        M::DeleteWarning => {
            "Les paramètres et mondes de cette instance seront définitivement supprimés. Sauvegardez vos données importantes avant de continuer."
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
        M::PackDownloaded => "{size} téléchargés.",
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
            "{name} ne démarre qu’après votre accord avec le CLUF de Minecraft. L’acceptez-vous ?"
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
        M::ModsOnlyCompatible => "N’afficher que les versions pour {gameVersion} / {loader}",
        M::ModsPlayersNeedToo => "Requis aussi pour les joueurs",
        M::ModsServerOnly => "Serveur uniquement",
        M::ModsPlayersNeed => "Requis pour les joueurs",
        M::ModsNeededBy => "Requis par {mods}",
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
        M::ModsMarkedForPlayers => {
            "Les joueurs recevront {name} la prochaine fois qu'ils rejoindront"
        }
        M::ModsUnmarkedForPlayers => "Les joueurs ne reçoivent plus {name} en rejoignant",
        M::ModsMarkFailed => "Impossible de modifier {name} : {error}",
        M::ModsRelease => "stable",
        M::ModsBeta => "bêta",
        M::ModsAlpha => "alpha",
        M::ModsResultInstalled => "Installé : {mods}",
        M::ModsResultRemoved => "Retiré : {files}",
        M::ModsResultSkipped => "Ignoré : {projects}",
        M::ModsResultPlayersNeed => "Les joueurs auront aussi besoin de {mods}",
        M::ModsByAuthor => "par {author}",
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
        M::ModsEnableFailed => "Impossible d’activer {name} : {error}",
        M::ModsDisableFailed => "Impossible de désactiver {name} : {error}",
        M::ModsUpdated => "{name} mis à jour vers {version}",
        M::ModsUpdateFailed => "Impossible de mettre à jour {name} : {error}",
        M::ModsUpdateTo => "Passer à {version}",
        M::ModsViewOnModrinth => "Voir sur Modrinth",
        M::ModsDeleteLabel => "Supprimer {name}",
        M::ModsUploaded => "Importé : {names}",
        M::ModsUploadFailed => "Échec de l’importation : {error}",
        M::ModsOnlyJars => "Seuls les fichiers .jar peuvent être importés : {names}",
        M::ModsUpdatesInstalled => "Mises à jour installées",
        M::ModsSomeUpdatesFailed => "Certaines mises à jour ont échoué : {failures}",
        M::ModsDeleted => "{name} supprimé",
        M::ModsDeleteFailed => "Échec de la suppression : {error}",
        M::ModsDeleteTitleMod => "Supprimer le mod ?",
        M::ModsDeleteTitlePlugin => "Supprimer le plugin ?",
        M::ModsDeleteBody => {
            "{name} ({fileName}) sera supprimé du serveur. Cette action est irréversible."
        }
        M::ModsOverwriteTitle => "Remplacer les fichiers existants ?",
        M::ModsOverwrite => "Remplacer",
        M::ModsOverwriteBody => "{message} Le remplacer par le fichier importé ?",
        M::ModsOverwriteDefault => "Un fichier du même nom est déjà installé.",
        M::ModsUpdateAll => "Tout mettre à jour ({count})",
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
        M::ModsLoader => "Fabric Loader {version}",
        M::ModsLoaderNoVersion => "Fabric Loader",
        M::ModsPlainFabric => {
            "Les joueurs peuvent se connecter avec Fabric seul (Minecraft {gameVersion}, {loader})."
        }
        M::ModsDownloadPack => "Télécharger le modpack client (.mrpack)",
        M::ModsPackShowFewer => "Afficher moins",
        M::ModsDownloadPackFailed => "Impossible de télécharger le modpack client : {error}",
        M::ModsPackSaved => "{name} enregistré",
        M::ModsSavePack => "Enregistrer",
        M::ModsImportHint => {
            "Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions."
        }
        M::ModpackFrom => "Ce serveur utilise le modpack {pack}.",
        M::ModpackCheck => "Rechercher une mise à jour",
        M::ModpackCheckFailed => "Impossible de rechercher une mise à jour : {error}",
        M::ModpackUpToDate => "C’est la version la plus récente.",
        M::ModpackUpdateTo => "Mettre à jour vers {version}",
        M::ModpackFromFile => "Mettre à jour depuis un fichier",
        M::ModpackFilePrompt => "Mettre à jour",
        M::ModpackNotMrpack => "{name} n’est pas un fichier .mrpack.",
        M::ModpackPlanFailed => "Impossible de préparer la mise à jour : {error}",
        M::ModpackDialogTitle => "Mettre à jour le modpack ?",
        M::ModpackDialogIntro => "{pack} passe de {from} à {to}.",
        M::ModpackDialogReinstall => "Le serveur passe à {software}, qui est réinstallé.",
        M::ModpackDialogKept => "Les fichiers que vous avez modifiés restent tels quels : {files}",
        M::ModpackDialogNothing => "Aucun fichier ne change.",
        M::ModpackDialogStop => {
            "Arrêtez d’abord le serveur : il ne peut pas être mis à jour pendant qu’il tourne."
        }
        M::ModpackDialogUnsupported => "Cette version ne peut pas être utilisée : {reason}",
        M::ModpackUpdate => "Mettre à jour",
        M::ModpackUpdated => "{pack} mis à jour vers {version}.",
        M::ModpackUpdateFailed => "Impossible de mettre à jour le modpack : {error}",
        M::ModpackUnknownVersion => "version inconnue",
        M::ModpackWhatsNew => "Nouveautés",
        M::ModpackOut => "{version} est disponible",
        M::ModpackHistory => "Historique",
        M::ModpackHistoryFailed => "Impossible de charger l’historique du modpack : {error}",
        M::ModpackHistoryEmpty => {
            "Rien n’est encore enregistré. Les prochaines mises à jour pourront être annulées."
        }
        M::ModpackKindInstall => "Installé",
        M::ModpackKindBaseline => "Premier enregistrement",
        M::ModpackKindUpdate => "Mis à jour",
        M::ModpackKindRestore => "{version} restauré",
        M::ModpackCurrent => "Actuelle",
        M::ModpackRestore => "Restaurer",
        M::ModpackRestoreMissing => "Lodestone n’a plus certains fichiers de cette version.",
        M::ModpackRestoreTitle => "Restaurer cette version ?",
        M::ModpackRestored => "{pack} {version} restauré.",
        M::ModpackRestoreFailed => "Impossible de restaurer le modpack : {error}",
        M::ModpackStoreSize => {
            "Pour annuler les mises à jour, Lodestone conserve {size} de fichiers qui ne peuvent pas être retéléchargés."
        }
        M::ModpackSince => "depuis {version}",
        M::ModpackSinceTooltip => "Ce fichier est arrivé avec la version {version} du modpack.",
        M::ModsProjectFailed => "Impossible de charger le projet : {error}",
        M::ModsSource => "Code source",
        M::ModsIssues => "Tickets",
        M::ModsWiki => "Wiki",
        M::ModsServerSide => "serveur : {value}",
        M::ModsSideRequired => "requis",
        M::ModsSideOptional => "facultatif",
        M::ModsSideUnsupported => "non pris en charge",
        M::ModsSideUnknown => "inconnu",
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
        M::ModsNoVersions => "Aucune version pour {loader} {gameVersion}.",
        M::ModsNoVersionsShowAll => {
            "Aucune version pour {loader} {gameVersion}. Activez « Afficher toutes les versions » pour voir les autres."
        }
        M::ModsNotFor => "Pas pour {loader} {gameVersion}",
        M::ModsInstalledVersion => "{title} {version} installé",
        M::ModsInstallFailed => "Impossible d’installer {title} : {error}",
        M::ModsIncompatibleTitle => "Installer une version incompatible ?",
        M::ModsInstallAnyway => "Installer quand même",
        M::ModsIncompatibleBody => {
            "{title} {version} est conçu pour {loaders} {gameVersions}, pas pour {loader} {gameVersion}. Le serveur risque de planter."
        }
        M::FriendsTitle => "Amis",
        M::FriendsLead => {
            "Les amis ajoutés dans Lodestone Launcher voient les serveurs que vous partagez avec eux et les rejoignent sans lien. Rien n’est partagé tant que vous ne cochez pas un ami."
        }
        M::FriendsAddInLauncher => "Ajoutez des amis dans Lodestone Launcher",
        M::FriendsAddInLauncherHint => {
            "Votre liste d’amis est gérée par Lodestone Launcher sur cet ordinateur. Une fois que vous y avez des amis, vous pouvez partager ce serveur avec eux ici."
        }
        M::FriendsLoading => "Chargement de vos amis…",
        M::FriendsShareWith => "Partager ce serveur avec {name}",
        M::FriendsNotVerified => "Non vérifié",
        M::FriendsMinecraft => {
            "Minecraft\u{a0}: {names}. Ajoutés à la liste blanche quand vous partagez, si le serveur l’a activée."
        }
        M::FriendsWhitelisted => "{names} ajouté(s) à la liste blanche du serveur",
        M::FriendsWhitelistFailed => {
            "Partagé avec {name}, mais impossible de l’ajouter à la liste blanche du serveur\u{a0}: {error}"
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
            "Vous n’avez pas encore comparé les codes de sécurité avec {name} : vous ne pouvez pas être sûr que c’est bien cette personne, quelqu’un d’autre a pu répondre à votre lien d’ami. Comparez d’abord les codes dans Lodestone Launcher, ou partagez quand même."
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
            "Vos amis ne voient plus ce core. Ce core ne peut pas créer lui-même un nouveau certificat : quittez Lodestone Host, supprimez {path}, puis rouvrez-le."
        }
        M::FriendsCancel => "Annuler",
        M::MemoryKeywords => "mémoire RAM",
        M::JvmArgsKeywords => "JVM Java arguments options",
        M::WhitelistKeywords => "liste blanche joueurs autorisés",
        M::MotdKeywords => "MOTD message du jour description du serveur",
        M::AutoStartKeywords => "démarrage lancement automatique",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::ModsDownloads => ["{count} téléchargement", "{count} téléchargements"],
        P::ModsFollowers => ["{count} abonné", "{count} abonnés"],
        P::ModsCountMod => [
            "{count} mod dans {directory}/",
            "{count} mods dans {directory}/",
        ],
        P::ModsCountPlugin => [
            "{count} plugin dans {directory}/",
            "{count} plugins dans {directory}/",
        ],
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
        P::ModsMoreGameVersions => ["+{count} autre", "+{count} autres"],
        P::ModpackAdded => ["{count} fichier ajouté", "{count} fichiers ajoutés"],
        P::ModpackReplaced => ["{count} fichier remplacé", "{count} fichiers remplacés"],
        P::ModpackRemoved => ["{count} fichier supprimé", "{count} fichiers supprimés"],
        P::ModpackKeptCount => [
            "{count} fichier modifié conservé",
            "{count} fichiers modifiés conservés",
        ],
        P::ModpackMoreChanges => ["et {count} autre", "et {count} autres"],
        P::FriendsConnectedCount => ["{count} ami connecté", "{count} amis connectés"],
    }
}
