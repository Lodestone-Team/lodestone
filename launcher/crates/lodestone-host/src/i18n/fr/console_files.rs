use crate::i18n::{ConsoleFilesMsg as M, ConsoleFilesPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::ConsoleSearch => "Rechercher dans la console…",
        M::ConsoleRegex => "Regex",
        M::ConsoleStatusNoPermission => "Aucune autorisation d’accès à la console",
        M::ConsoleStatusLoading => "Chargement de la console…",
        M::ConsoleStatusBuffered => "Historique des messages. Pas de mises à jour en direct",
        M::ConsoleStatusLive => "Console en direct",
        M::ConsoleStatusLiveNoBuffer => {
            "Console en direct, mais impossible de récupérer l’historique. Votre connexion Internet est peut-être instable"
        }
        M::ConsoleStatusClosed => "La console est fermée",
        M::ConsoleStatusError => "Connexion perdue ou erreur",
        M::ConsoleServerStopped => "Le serveur est arrêté",
        M::ConsoleInputNoPermission => "Aucune autorisation",
        M::ConsoleInputPlaceholder => "Saisir une commande…",
        M::ConsoleNoPermission => "Vous n’avez pas l’autorisation d’accéder à cette console",
        M::ConsoleLoading => "Chargement de la console",
        M::ConsoleEmpty => "Aucun message dans la console pour l’instant",
        M::ConsoleSendFailed => "Impossible d’envoyer la commande : {error}",
        M::ConsoleLinkedUser => "L’utilisateur Lodestone qui a associé ce compte Minecraft",
        M::ConsoleOwnerOnlyAtom => "Seul le propriétaire peut contrôler les serveurs atom",
        M::ConsoleJumpToLatest => "Aller à la fin",
        M::Console => "Console",
        M::ConsoleCommand => "Commande",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {}
}
