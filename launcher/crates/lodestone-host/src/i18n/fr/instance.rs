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
            "Les paramètres, mondes et sauvegardes de cette instance seront définitivement supprimés. Sauvegardez vos données importantes avant de continuer."
        }
        M::Deleting => "Suppression de {name}…",
        M::OwnerOnlyAtom => "Seul le propriétaire peut contrôler les serveurs atom.",
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
        M::PlayitLoadFailed => "Impossible de charger l’état de playit.gg : {error}",
        M::PlayitSignupFailed => "Impossible de générer un lien d’inscription : {error}",
        M::PlayitVerifyFailed => "Impossible de vérifier le compte playit.gg : {error}",
        M::PlayitStartFailed => "Impossible de démarrer l’agent playit.gg : {error}",
        M::PlayitStopFailed => "Impossible d’arrêter l’agent playit.gg : {error}",
        M::PlayitSetupTitle => "Configurer playit.gg",
        M::PlayitSetupIntro => {
            "Inscrivez-vous à playit.gg pour l’utiliser. Générez un lien d’inscription avec le bouton ci-dessous."
        }
        M::PlayitGenerateLink => "Générer un lien d’inscription",
        M::PlayitFollowLink => {
            "Ouvrez le lien et créez un compte. Quand vous voyez le tableau de bord de playit.gg, revenez et appuyez sur Vérifier."
        }
        M::PlayitOpenLink => "Ouvrir le lien d’inscription",
        M::PlayitVerify => "Vérifier",
        M::PlayitRunner => "Agent playit.gg",
        M::PlayitRunnerLead => {
            "Fait fonctionner vos tunnels sur cet ordinateur, pour que les joueurs rejoignent sans redirection de port."
        }
        M::PlayitStopped => "Arrêté",
        M::PlayitLoading => "Démarrage",
        M::PlayitStarted => "Démarré",
        M::PlayitTrouble => {
            "Un problème ? Vérifiez que le tunnel est configuré sur le site de playit.gg et que votre serveur est en marche."
        }
        M::PlayitSignupRequired => "Inscrivez-vous à playit.gg avant de l’utiliser.",
        M::PlayitSignUp => "S’inscrire",
        M::PlayitTunnels => "Tunnels",
        M::PlayitTunnelsLead => "Créez d’autres tunnels et modifiez-les sur le site de playit.gg.",
        M::PlayitWebsite => "Ouvrir playit.gg",
        M::PlayitLocalServer => "Serveur local à l’adresse {address}",
        M::PlayitCopyAddress => "Copier l’adresse",
        M::PlayitCopied => "{address} copié",
        M::PlayitActive => "Actif",
        M::PlayitInactive => "Inactif",
        M::PlayitNoTunnels => "Vous n’avez encore créé aucun tunnel",
        M::PlayitNotSignedUp => "Vous n’êtes pas encore inscrit",
        M::PlayitHelp => "Besoin d’aide ?",
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
            "Modrinth n’indique pas si les joueurs en ont besoin. Consultez la page du mod."
        }
        M::ModsUnknownNotOnModrinth => "Absent de Modrinth. Consultez la page du mod.",
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
        M::ModsDownloadPackFailed => "Impossible de télécharger le modpack client : {error}",
        M::ModsPackSaved => "{name} enregistré",
        M::ModsSavePack => "Enregistrer",
        M::ModsImportHint => {
            "Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions."
        }
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
        P::ModsShowMore => [
            "Afficher plus ({count} restante)",
            "Afficher plus ({count} restantes)",
        ],
        P::ModsMoreGameVersions => ["+{count} autre", "+{count} autres"],
    }
}
