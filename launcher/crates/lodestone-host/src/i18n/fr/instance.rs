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
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
