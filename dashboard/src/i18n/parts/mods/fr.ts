import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // utils/mods
  'mods.tab.mods': 'Mods',
  'mods.tab.plugins': 'Plugins',

  // ModsTab
  'mods.pageTitle.mod': 'Mods du serveur',
  'mods.pageTitle.plugin': 'Plugins du serveur',
  'mods.unsupported':
    'Ce serveur ne prend pas en charge les mods ni les plugins. Les serveurs Fabric, Forge, Paper et Spigot, si.',
  'mods.noReadPermission':
    'Vous n’avez pas l’autorisation de lire les fichiers de cette instance.',
  'mods.view.installed': 'Installés',
  'mods.view.browse': 'Parcourir',
  'mods.restartRequired.mod':
    'Redémarrez le serveur pour appliquer les modifications des mods',
  'mods.restartRequired.plugin':
    'Redémarrez le serveur pour appliquer les modifications des plugins',
  'mods.browse.placeholder.mod':
    'Rechercher des mods sur Modrinth, ou coller un lien Modrinth',
  'mods.browse.placeholder.plugin':
    'Rechercher des plugins sur Modrinth, ou coller un lien Modrinth',
  'mods.browse.onlyCompatible':
    'N’afficher que les versions pour {gameVersion} / {loader}',

  // common
  'mods.noWritePermission':
    'Vous n’avez pas l’autorisation de modifier les fichiers de ce serveur',
  'mods.clientSide.playersNeedToo': 'Requis aussi pour les joueurs',
  'mods.clientSide.serverOnly': 'Serveur uniquement',
  'mods.clientRequirement.required': 'Requis pour les joueurs',
  'mods.clientRequirement.neededBy': 'Requis par {mods}',
  'mods.clientRequirement.unknownModrinth':
    'Modrinth n’indique pas si les joueurs en ont besoin ; consultez la page du mod',
  'mods.clientRequirement.unknownNotOnModrinth':
    'Absent de Modrinth ; consultez la page du mod',
  'mods.versionType.release': 'stable',
  'mods.versionType.beta': 'bêta',
  'mods.versionType.alpha': 'alpha',
  'mods.installResult.installed': 'Installé : {mods}',
  'mods.installResult.removed': 'Retiré : {files}',
  'mods.installResult.skipped': 'Ignoré : {projects}',
  'mods.installResult.playersNeed': 'Les joueurs auront aussi besoin de {mods}',
  'mods.byAuthor': 'par {author}',
  'mods.installedLabel': 'Installé',
  'mods.downloads': {
    one: '{count} téléchargement',
    other: '{count} téléchargements',
  },
  'mods.followers': { one: '{count} abonné', other: '{count} abonnés' },

  // ModrinthSearch
  'mods.sort.label': 'Trier par',
  'mods.sort.relevance': 'Pertinence',
  'mods.sort.downloads': 'Téléchargements',
  'mods.sort.follows': 'Abonnés',
  'mods.sort.newest': 'Plus récents',
  'mods.sort.updated': 'Mis à jour récemment',
  'mods.search.ariaLabel': 'Rechercher sur Modrinth',
  'mods.search.opening': 'Ouverture du lien…',
  'mods.search.linkFailed': 'Impossible d’ouvrir le lien : {error}',
  'mods.search.openLink': 'Ouvrir {slug}',
  'mods.search.failed': 'Échec de la recherche : {error}',
  'mods.search.loading': 'Chargement des résultats',
  'mods.search.noResults': 'Aucun résultat.',
  'mods.search.page': 'Page {page} sur {total}',
  'mods.search.previousPage': 'Page précédente',
  'mods.search.nextPage': 'Page suivante',

  // InstalledMods
  'mods.installed.enableFailed': 'Impossible d’activer {name} : {error}',
  'mods.installed.disableFailed': 'Impossible de désactiver {name} : {error}',
  'mods.installed.updated': '{name} mis à jour vers {version}',
  'mods.installed.updateFailed': 'Impossible de mettre à jour {name} : {error}',
  'mods.installed.updateTo': 'Passer à {version}',
  'mods.installed.viewOnModrinth': 'Voir sur Modrinth',
  'mods.installed.deleteLabel': 'Supprimer {name}',
  'mods.installed.uploaded': 'Importé : {names}',
  'mods.installed.uploadFailed': 'Échec de l’importation : {error}',
  'mods.installed.onlyJars':
    'Seuls les fichiers .jar peuvent être importés : {names}',
  'mods.installed.updatesInstalled': 'Mises à jour installées',
  'mods.installed.someUpdatesFailed':
    'Certaines mises à jour ont échoué : {failures}',
  'mods.installed.deleted': '{name} supprimé',
  'mods.installed.deleteFailed': 'Échec de la suppression : {error}',
  'mods.installed.deleteTitle.mod': 'Supprimer le mod ?',
  'mods.installed.deleteTitle.plugin': 'Supprimer le plugin ?',
  'mods.installed.deleteBody':
    '{name} ({fileName}) sera supprimé du serveur. Cette action est irréversible.',
  'mods.installed.overwriteTitle': 'Remplacer les fichiers existants ?',
  'mods.installed.overwrite': 'Remplacer',
  'mods.installed.overwriteBody':
    '{message} Le remplacer par le fichier importé ?',
  'mods.installed.overwriteDefault':
    'Un fichier du même nom est déjà installé.',
  'mods.installed.count.mod': {
    one: '{count} mod dans {directory}/',
    other: '{count} mods dans {directory}/',
  },
  'mods.installed.count.plugin': {
    one: '{count} plugin dans {directory}/',
    other: '{count} plugins dans {directory}/',
  },
  'mods.installed.updateAll': 'Tout mettre à jour ({count})',
  'mods.installed.checkUpdates': 'Rechercher des mises à jour',
  'mods.installed.upload': 'Importer',
  'mods.installed.loadFailed.mod': 'Impossible de charger les mods : {error}',
  'mods.installed.loadFailed.plugin':
    'Impossible de charger les plugins : {error}',
  'mods.installed.loading': 'Chargement des mods',
  'mods.installed.empty.mod':
    'Aucun mod installé pour l’instant. Parcourez Modrinth ou importez un fichier .jar.',
  'mods.installed.empty.plugin':
    'Aucun plugin installé pour l’instant. Parcourez Modrinth ou importez un fichier .jar.',

  // ClientPackPanel
  'mods.clientPack.loader': 'Fabric Loader {version}',
  'mods.clientPack.loaderNoVersion': 'Fabric Loader',
  'mods.clientPack.plainFabric':
    'Les joueurs peuvent se connecter avec Fabric seul (Minecraft {gameVersion}, {loader}).',
  'mods.clientPack.title': {
    one: 'Les joueurs ont besoin de {count} mod pour se connecter',
    other: 'Les joueurs ont besoin de {count} mods pour se connecter',
  },
  'mods.clientPack.download': 'Télécharger le modpack client (.mrpack)',
  'mods.clientPack.downloadFailed':
    'Impossible de télécharger le modpack client : {error}',
  'mods.clientPack.importHint':
    'Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions.',
  'mods.clientPack.importHintWithJars': {
    one: 'Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions. {count} mod n’est pas sur Modrinth, son fichier est donc inclus dans le pack.',
    other:
      'Importez-le dans Prism Launcher, l’application Modrinth ou ATLauncher pour tout obtenir dans les bonnes versions. {count} mods ne sont pas sur Modrinth, leurs fichiers sont donc inclus dans le pack.',
  },

  // ModrinthProjectView
  'mods.project.loading': 'Chargement du projet',
  'mods.project.loadFailed': 'Impossible de charger le projet : {error}',
  'mods.project.link.source': 'Code source',
  'mods.project.link.issues': 'Tickets',
  'mods.project.link.wiki': 'Wiki',
  'mods.project.serverSide': 'serveur : {value}',
  'mods.project.side.required': 'requis',
  'mods.project.side.optional': 'facultatif',
  'mods.project.side.unsupported': 'non pris en charge',
  'mods.project.side.unknown': 'inconnu',
  'mods.project.createFromModpack': 'Créer un serveur à partir de ce modpack',
  'mods.project.modpackNoPermission':
    'Ceci est un modpack. Vous n’avez pas l’autorisation de créer des serveurs.',
  'mods.project.noServerSupport':
    'D’après ce projet, il ne fonctionne pas sur les serveurs.',
  'mods.project.wrongType.mod':
    'Ce projet est de type {type}. Lodestone n’installe ici que des mods.',
  'mods.project.wrongType.plugin':
    'Ce projet est de type {type}. Lodestone n’installe ici que des plugins.',
  'mods.project.description': 'Description',
  'mods.versions.title': 'Versions',
  'mods.versions.withDependencies': 'Installer aussi les dépendances requises',
  'mods.versions.showAll': 'Afficher toutes les versions',
  'mods.versions.loadFailed': 'Impossible de charger les versions : {error}',
  'mods.versions.loading': 'Chargement des versions',
  'mods.versions.none': 'Aucune version pour {loader} {gameVersion}.',
  'mods.versions.noneShowAll':
    'Aucune version pour {loader} {gameVersion}. Activez « Afficher toutes les versions » pour voir les autres.',
  'mods.versions.showMore': {
    one: 'Afficher plus ({count} restante)',
    other: 'Afficher plus ({count} restantes)',
  },
  'mods.version.moreGameVersions': {
    one: '+{count} autre',
    other: '+{count} autres',
  },
  'mods.version.notFor': 'Pas pour {loader} {gameVersion}',
  'mods.version.install': 'Installer',
  'mods.version.installed': '{title} {version} installé',
  'mods.version.installFailed': 'Impossible d’installer {title} : {error}',
  'mods.version.incompatibleTitle': 'Installer une version incompatible ?',
  'mods.version.installAnyway': 'Installer quand même',
  'mods.version.incompatibleBody':
    '{title} {version} est conçu pour {loaders} {gameVersions}, pas pour {loader} {gameVersion}. Le serveur risque de planter.',
} satisfies CatalogOf<typeof en>;
