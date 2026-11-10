use crate::i18n::{FriendListMsg as M, FriendListPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::Title => "Amis",
        M::Hint => {
            "Les personnes ajoutées une fois pour toutes. Voyez et rejoignez les serveurs qu’elles partagent avec vous, sans lien."
        }
        M::AddFriend => "Ajouter un ami",
        M::Refreshing => "Vérification de vos amis…",
        M::You => "Vous",
        M::YourSafetyCode => "Votre code de sécurité\u{a0}: {code}",
        M::NoNameYet => {
            "Choisissez le nom que verront vos amis, puis ajoutez-les avec un lien d’ami."
        }
        M::ChooseName => "Choisir un nom",
        M::EditName => "Modifier le nom",
        M::OpenFailed => "Impossible d’ouvrir votre liste d’amis",
        M::ActionFailed => "Impossible de mettre à jour votre liste d’amis",
        M::NoFriendsYet => {
            "Pas encore d’amis. Créez un lien d’ami et envoyez-le à quelqu’un que vous connaissez."
        }
        M::RefreshProblem => "Certains amis n’ont pas pu être vérifiés\u{a0}: {error}",
        M::LinksTitle => "Vos liens d’ami",
        M::LinkWaiting => "En attente d’une réponse. Utilisable une fois, jusqu’à {time}.",
        M::CancelLink => "Annuler le lien",
        M::CheckAnswers => "Vérifier les réponses",
        M::ReplyFrom => "{name} a répondu à votre lien d’ami",
        M::SafetyCode => "Code de sécurité\u{a0}: {code}",
        M::SameNameAs => {
            "Vous avez déjà un ami nommé {names}. Assurez-vous qu’il s’agit bien de quelqu’un d’autre avant de continuer."
        }
        M::Accept => "Accepter",
        M::Ignore => "Ignorer",
        M::WaitingFor => "En attente de la confirmation de {name}",
        M::NotVerified => "Non vérifié",
        M::Verified => "Codes identiques",
        M::Hidden => "Masqué pour cet ami",
        M::Retired => "N’utilise plus cette liste d’amis",
        M::TheirName => "Son nom\u{a0}: {name}",
        M::OnlineOn => "En ligne\u{a0}: {cores}",
        M::Offline => "Hors ligne",
        M::SharesNothing => "Ne partage encore aucun serveur avec vous",
        M::Servers => "Serveurs",
        M::CompareCodes => "Comparer les codes",
        M::HideFromThem => "Me masquer",
        M::StopHiding => "Ne plus me masquer",
        M::CoreOnline => "En ligne",
        M::CoreOffline => "Hors ligne",
        M::LoadingServers => "Demande des serveurs…",
        M::NoServersShared => "Rien sur cet ordinateur n’est partagé avec vous.",
        M::ServersFailed => "Impossible de lister les serveurs\u{a0}: {error}",
        M::ShowServers => "Afficher les serveurs",
        M::AddTitle => "Ajouter un ami",
        M::SendLinkTitle => "Envoyez-lui un lien d’ami",
        M::SendLinkBody => {
            "Créez un lien et envoyez-le à votre ami. Quand il l’ouvrira, sa réponse apparaîtra sur la page Serveurs pour que vous l’acceptiez."
        }
        M::MakeLink => "Créer un lien",
        M::LinkWorksOnce => {
            "Utilisable une fois, pendant 24\u{a0}heures. Toute personne qui a ce lien peut demander à devenir votre ami\u{202F}: ne l’envoyez qu’à lui."
        }
        M::OpenLinkTitle => "Ou ouvrez son lien d’ami",
        M::LinkPlaceholder => "Lien d’ami (https://connect.lodestone.cc/f/…)",
        M::LookUp => "Consulter",
        M::NotAFriendLink => {
            "Ce n’est pas un lien d’ami. Les liens d’ami commencent par https://connect.lodestone.cc/f/."
        }
        M::PasteFirst => "Collez d’abord un lien d’ami.",
        M::OtherService => {
            "Ce lien est destiné à {host}, qui n’est pas le service Lodestone Connect qu’utilise ce lanceur\u{202F}: il ne peut pas être ouvert ici."
        }
        M::PreviewFrom => "Ce lien vient de {name}",
        M::PreviewCompare => {
            "Une fois amis, comparez vos codes de sécurité avec {name} de vive voix ou en personne. N’ajoutez que des personnes que vous connaissez."
        }
        M::AlreadyFriends => "Vous êtes déjà ami avec {name}.",
        M::Added => {
            "{name} est dans votre liste d’amis. Rien n’est partagé tant qu’il n’a pas confirmé."
        }
        M::NeedNameFirst => {
            "Choisissez d’abord votre nom\u{a0}: votre ami le verra avec votre réponse."
        }
        M::NameTitle => "Votre nom",
        M::NameBody => {
            "Vos amis voient ce nom. Si vous le changez, ils verront le nouveau la prochaine fois que leur lanceur vérifiera."
        }
        M::NamePlaceholder => "Votre nom",
        M::NameEmpty => "Choisissez un nom que verront vos amis.",
        M::VerifyTitle => "Comparer les codes de sécurité avec {name}",
        M::VerifyBody => {
            "Appelez {name} ou retrouvez-le, et lisez-vous vos codes. S’ils sont identiques, personne ne s’est interposé quand vous êtes devenus amis. Ne les comparez pas dans la discussion par laquelle le lien est passé."
        }
        M::YourCode => "Votre code",
        M::TheirCode => "Code de {name}",
        M::CodesMatch => "Les codes sont identiques",
        M::RenameTitle => "Renommer {name}",
        M::RenameBody => "Vous seul voyez ce nom. Le nom qu’il a choisi est {name}.",
        M::NicknamePlaceholder => "Surnom",
        M::UseTheirName => "Utiliser son nom",
        M::Retire => "Retirer l’identité",
        M::RetireTitle => "Retirer votre identité d’ami\u{202F}?",
        M::RetireBody => {
            "Chacun de vos amis est prévenu que cette identité est retirée\u{202F}: ils vous retirent et cessent de faire confiance aux mods de vos serveurs. Ensuite, votre clé, votre liste d’amis et vos liens d’ami en cours sont supprimés de cet ordinateur. Faites-le si votre clé a pu être volée, ou pour repartir de zéro."
        }
        M::RetireWarning => {
            "C’est irréversible. Pour redevenir ami avec quelqu’un, vous créez une nouvelle identité et l’ajoutez de nouveau avec de nouveaux liens d’ami."
        }
        M::RetireUnderstood => {
            "J’ai compris\u{a0}: tous mes amis sont retirés, et c’est irréversible"
        }
        M::RetireConfirm => "Retirer l’identité",
        M::SharingFrom => "Partage depuis {core}",
        M::RetireCore => "Retirer",
        M::RetireCoreTitle => "Retirer {name}\u{202F}?",
        M::RetireCoreBody => {
            "Vos amis sont prévenus d’oublier {name} : ils ne voient plus ses serveurs et ne font plus confiance à leurs mods. Faites-le pour un ordinateur perdu ou volé, ou depuis lequel vous ne partagez plus."
        }
        M::RetireCoreWarning => {
            "C’est irréversible. Ce core n’est plus jamais montré à vos amis, même s’il se synchronise. Pour partager de nouveau depuis cet ordinateur, il lui faut un nouveau certificat d’ami : Lodestone Host sur cet ordinateur en propose un dans les Paramètres d’un serveur, sous Amis."
        }
        M::RetireCoreConfirm => "Retirer le core",
        M::NameLike => "Même nom que {names} : vérifiez de qui il s’agit",
        M::RemoveTitle => "Retirer {name}\u{202F}?",
        M::RemoveBody => {
            "Vous ne voyez plus ses serveurs, et il ne voit plus les vôtres. Vos cores cessent de partager avec lui la prochaine fois que Lodestone Host les met à jour. Pour redevenir amis, l’un de vous crée un nouveau lien d’ami."
        }
        M::RemoveFriend => "Retirer cet ami",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::AnsweredTimes => [
            "Ce lien a reçu {count} réponse. Quelqu’un d’autre a peut-être ce lien\u{a0}: comparez les codes de sécurité avant d’accepter.",
            "Ce lien a reçu {count} réponses. Quelqu’un d’autre a peut-être ce lien\u{a0}: comparez les codes de sécurité avant d’accepter.",
        ],
    }
}
