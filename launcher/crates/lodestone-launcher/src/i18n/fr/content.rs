use crate::i18n::{ContentMsg as M, ContentPlural as P};

pub fn text(message: M) -> &'static str {
    match message {
        M::KindMods => "Mods",
        M::KindResourcePacks => "Packs de ressources",
        M::KindShaders => "Shaders",
        M::Vanilla => "Vanilla",
        M::SearchInstalledOrModrinth => "Rechercher",
        M::InstalledCount => "Installés · {count}",
        M::EnabledCount => "Activés · {count} · le pack du haut l’emporte en cas de chevauchement",
        M::AvailableCount => "Disponibles · {count}",
        M::OnModrinthCount => "Sur Modrinth · {count}",
        M::OnModrinth => "Sur Modrinth",
        M::OnCurseForgeCount => "Sur CurseForge · {count}",
        M::OnCurseForge => "Sur CurseForge",
        M::NoInstalledMatch => {
            "Installés · {count} · Aucun élément installé ne correspond ({kind})."
        }
        M::OnlyTarget => "{target} uniquement",

        M::StopGameFirst => {
            "Arrêtez d’abord le jeu\u{a0}: il réécrit ces fichiers lorsqu’il se ferme."
        }
        M::WaitForInstall => "Attendez la fin de l’installation.",
        M::GameRunning => {
            "Le jeu est en marche. Arrêtez-le pour modifier son contenu\u{a0}: il réécrit options.txt et les réglages des shaders lorsqu’il se ferme."
        }

        M::AllModsUpToDate => "Tous les mods venant de Modrinth sont à jour.",
        M::AllResourcePacksUpToDate => {
            "Tous les packs de ressources venant de Modrinth sont à jour."
        }
        M::AllShaderPacksUpToDate => "Tous les packs de shaders venant de Modrinth sont à jour.",
        M::CouldntCheckUpdates => "Impossible de rechercher les mises à jour\u{a0}: {error}",

        M::CheckingProject => "Vérification de {name}",
        M::CheckingDependencies => "Vérification des dépendances",
        M::FilesProgress => "{done} fichiers sur {total}",
        M::BytesProgress => "{done} / {total}",

        M::NothingInstalled => "Rien n’a été installé.",
        M::InstalledOne => "{name} installé.",
        M::InstalledMany => "{names} et {last} installés.",
        M::Replaced => "Remplacé\u{a0}: {files}.",
        M::Skipped => "{name} ignoré\u{a0}: {reason}.",

        M::AddedFiles => "Ajouté\u{a0}: {files}.",
        M::FileError => "{name}\u{a0}: {error}",
        M::NotAddedFiles => "Non ajouté\u{a0}: {files}",
        M::AddedAndNotAdded => "Ajouté\u{a0}: {added}. Non ajouté\u{a0}: {failed}",
        M::AddToMods => "Ajouter aux mods",
        M::AddToResourcePacks => "Ajouter aux packs de ressources",
        M::AddToShaders => "Ajouter aux shaders",

        M::LinkNotInstallable => {
            "Ce lien mène à un projet de type «\u{a0}{kind}\u{a0}», que Lodestone Launcher n’installe pas encore."
        }

        M::CheckForUpdates => "Rechercher des mises à jour",
        M::UpdateAll => "Tout mettre à jour ({count})",
        M::AddFile => "Ajouter un fichier…",
        M::ItemFolder => "{name} · dossier",
        M::Compatible => "Compatible",
        M::OlderVersion => "Version plus ancienne",
        M::NewerVersion => "Version plus récente",
        M::NotOnModrinth => "Absent de Modrinth",
        M::UpdateTo => "Passer à {version}",
        M::NoModsYet => {
            "Aucun mod pour l’instant. Parcourez Modrinth, ou déposez des fichiers .jar ici."
        }
        M::NoResourcePacksYet => {
            "Aucun pack de ressources pour l’instant. Parcourez Modrinth, ou déposez ici des fichiers .zip ou des dossiers."
        }
        M::NoShaderPacksYet => {
            "Aucun pack de shaders pour l’instant. Parcourez Modrinth, ou déposez ici des fichiers .zip ou des dossiers."
        }
        M::ReadingFolder => "Lecture du dossier {folder}",
        M::CouldntReadFolder => "Impossible de lire le dossier {folder}\u{a0}: {error}",
        M::SomeFilesNotMatched => {
            "Certains fichiers n’ont pas pu être associés à Modrinth\u{a0}: {error}"
        }
        M::ShadersOff => "Shaders désactivés",
        M::NormalLook => "L’apparence normale de Minecraft.",

        M::ModsNeedLoader => "Les mods ont besoin d’un chargeur de mods",
        M::ShadersNeedLoader => "Les shaders ont besoin d’un chargeur de mods",
        M::VanillaSwitchMods => {
            "Cette instance est un Minecraft vanilla. Les mods passent par un chargeur de mods comme {loader}\u{202F}; changer de chargeur conserve vos mondes et vos paramètres, et {loader} s’installera la prochaine fois que vous appuierez sur Jouer."
        }
        M::VanillaSwitchShaders => {
            "Cette instance est un Minecraft vanilla. Les shaders passent par un chargeur de mods comme {loader}\u{202F}; changer de chargeur conserve vos mondes et vos paramètres, et {loader} s’installera la prochaine fois que vous appuierez sur Jouer."
        }
        M::VanillaNoLoaderMods => {
            "Cette instance est un Minecraft {version} vanilla. Les mods ont besoin d’un chargeur de mods, et aucun de ceux que Lodestone Launcher prend en charge ne fonctionne sur cette version\u{a0}: Fabric commence à la 1.14, et Forge n’a pas de build pour elle. Une nouvelle instance sur une version plus récente pourra les utiliser."
        }
        M::VanillaNoLoaderShaders => {
            "Cette instance est un Minecraft {version} vanilla. Les shaders ont besoin d’un chargeur de mods, et aucun de ceux que Lodestone Launcher prend en charge ne fonctionne sur cette version\u{a0}: Fabric commence à la 1.14, et Forge n’a pas de build pour elle. Une nouvelle instance sur une version plus récente pourra les utiliser."
        }
        M::VanillaCheckFailedMods => {
            "Cette instance est un Minecraft vanilla. Les mods ont besoin d’un chargeur de mods, mais impossible de vérifier lequel convient\u{a0}: {error}"
        }
        M::VanillaCheckFailedShaders => {
            "Cette instance est un Minecraft vanilla. Les shaders ont besoin d’un chargeur de mods, mais impossible de vérifier lequel convient\u{a0}: {error}"
        }
        M::VanillaCheckingMods => {
            "Cette instance est un Minecraft vanilla. Les mods passent par un chargeur de mods\u{202F}; recherche de celui qui fonctionne sur Minecraft {version}…"
        }
        M::VanillaCheckingShaders => {
            "Cette instance est un Minecraft vanilla. Les shaders passent par un chargeur de mods\u{202F}; recherche de celui qui fonctionne sur Minecraft {version}…"
        }
        M::SwitchTo => "Passer à {loader}",
        M::NewFabricInstance => "Nouvelle instance Fabric…",

        M::ShaderModUsingPack => "{shader_mod} utilise {pack}.",
        M::ShaderModUsingBuiltIn => "{shader_mod} utilise ses shaders intégrés.",
        M::ShaderModInstalledOff => {
            "{shader_mod} est installé\u{202F}; les shaders sont désactivés."
        }
        M::ShaderModPickPack => {
            "Choisissez un pack ci-dessous. {shader_mod} lit ce choix dans {file} au démarrage du jeu."
        }
        M::ShadersNeedShaderMod => "Les shaders ont besoin d’un mod de shaders",
        M::OptiFineNote => {
            "OptiFine fonctionne aussi, mais il n’est pas sur Modrinth et sa licence ne permet pas à Lodestone de le télécharger. Si vous le préférez, téléchargez-le sur optifine.net et ajoutez le fichier .jar avec {add_file} dans {mods} (Fabric a aussi besoin d’OptiFabric)."
        }
        M::LookingForShaderMod => "Recherche d’un mod de shaders pour cette version sur Modrinth",
        M::CouldntAskModrinth => "Impossible d’interroger Modrinth\u{a0}: {error}",
        M::InstallShaderMod => "Installer {shader_mod} {version}",
        M::ShaderModRunsOn => {
            "Minecraft ne peut pas charger de packs de shaders tout seul\u{202F}; c’est le rôle d’un mod de shaders. {shader_mod} fonctionne avec le chargeur {loader} de cette instance."
        }
        M::ShaderModComesWith => "Il est accompagné de {dependency}, dont {shader_mod} a besoin.",
        M::ShaderModUnavailable => {
            "Minecraft ne peut pas charger de packs de shaders tout seul\u{202F}; c’est le rôle d’un mod de shaders. {reason} Les instances Fabric peuvent toujours utiliser Iris."
        }

        M::BrowsingNeedsNetwork => "Parcourir Modrinth nécessite une connexion réseau.",
        M::SortRelevance => "Pertinence",
        M::SortDownloads => "Téléchargements",
        M::SortFollows => "Abonnés",
        M::SortNewest => "Récents",
        M::SortUpdated => "Mis à jour",
        M::SearchingModrinth => "Recherche sur Modrinth",
        M::NothingMatches => "Aucun résultat sur Modrinth.",
        M::NothingOnCurseForge => "Aucun résultat sur CurseForge.",
        M::PageOf => "Page {page} sur {pages}",
        M::ByAuthor => "par {author}",
        M::InstalledBadge => "Installé",
        M::OptionalOnServers => "Facultatif sur les serveurs",
        M::DownloadsCompact => "{count} téléchargements",
        M::FollowsCompact => "{count} abonnés",
        M::FollowersCompact => "{count} abonnés",

        M::LoadingProject => "Chargement du projet",
        M::ViewOnModrinth => "Voir sur Modrinth",
        M::Versions => "Versions",
        M::Description => "Description",
        M::LoadingVersions => "Chargement des versions",
        M::ShowAllVersions => {
            "Afficher les versions pour d’autres chargeurs et versions de Minecraft"
        }
        M::AlsoInstallDependencies => "Installer aussi les dépendances requises",
        M::NoVersionFits => {
            "Aucune version de {project} ne convient à cette instance. Activez «\u{a0}{show_all}\u{a0}» pour toutes les voir."
        }
        M::ShowVersionsShort => "Afficher les versions…",
        M::InstallAnyway => "Installer quand même…",
        M::InstallItAnyway => "L’installer quand même",
        M::InstallWith => "Installer avec {shader_mod}",
        M::Release => "Stable",
        M::Beta => "Bêta",
        M::Alpha => "Alpha",

        M::ModrinthTab => "Modrinth",
        M::CurseForgeTab => "CurseForge",
        M::SearchingCurseForge => "Recherche sur CurseForge",
        M::ViewOnCurseForge => "Voir sur CurseForge",
        M::NotOnEitherPlatform => "Absent de Modrinth et de CurseForge",
        M::SortPopular => "Populaires",
        M::SortName => "Nom",
        M::WebsiteDownload => "Sur le site uniquement",
        M::GetOnCurseForge => "L’obtenir sur CurseForge",
        M::BlockedTitle => "À télécharger depuis CurseForge",
        M::BlockedLead => {
            "Leurs auteurs ne permettent pas aux lanceurs de les télécharger. Téléchargez-les dans votre navigateur\u{a0}: Lodestone les ajoute dès qu’ils apparaissent dans {folder}."
        }
        M::BlockedNoFolder => {
            "Leurs auteurs ne permettent pas aux lanceurs de les télécharger. Téléchargez-les dans votre navigateur, puis choisissez le dossier où ils ont été enregistrés."
        }
        M::BlockedOpen => "Ouvrir la page de téléchargement",
        M::BlockedOpenAll => "Ouvrir toutes les pages",
        M::BlockedAdded => "Ajouté",
        M::BlockedWaiting => "En attente de {file}",
        M::BlockedChangeFolder => "Surveiller un autre dossier…",
        M::BlockedSkip => "Ignorer le reste",
        M::WatchFolderPrompt => "Surveiller ce dossier",
        M::CouldntAddDownload => "Impossible d’ajouter {name}\u{a0}: {error}",
        M::AddedDownloads => "{names} ajouté(s).",
    }
}

pub fn plural(message: P) -> [&'static str; 2] {
    match message {
        P::Downloads => ["{count} téléchargement", "{count} téléchargements"],
        P::Follows => ["{count} abonné", "{count} abonnés"],
        P::Followers => ["{count} abonné", "{count} abonnés"],
        P::MoreVersions => [
            "{count} autre version est sur Modrinth.",
            "{count} autres versions sont sur Modrinth.",
        ],
    }
}
