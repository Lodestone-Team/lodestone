import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // tabList
  'server.tab.overview': 'Vue d’ensemble',
  'server.tab.settings': 'Paramètres',
  'server.tab.console': 'Console',
  'server.tab.files': 'Fichiers',
  'server.tab.mods': 'Mods',
  'server.tab.macros': 'Macros',
  'server.tab.logs': 'Journal d’événements',
  'server.tab.playitgg': 'Playitgg',

  // InstanceTabs
  'server.notFound': 'Serveur introuvable',
  'server.tabNotFound': '{path} introuvable',
  'server.logs.comingSoon': 'Bientôt disponible dans votre tableau de bord !',

  // InstanceOverview
  'server.overview.pageTitle': 'Vue d’ensemble du serveur',
  'server.overview.noName': 'Sans nom',
  'server.overview.noDescription': 'Aucune description',
  'server.overview.version': 'Version {version}',
  'server.overview.playerCount': 'Joueurs {players}/{max}',

  // InstanceDetails
  'server.details.title': 'Détails',
  'server.details.name': 'Nom',
  'server.details.description': 'Description',
  'server.details.uuid': 'UUID',
  'server.details.game': 'Jeu',
  'server.details.minecraftVersion': 'Version de Minecraft',
  'server.details.version': 'Version',
  'server.details.loader': 'Chargeur',
  'server.details.port': 'Port',
  'server.details.state': 'État',
  'server.details.players': 'Joueurs',
  'server.details.created': 'Créé le',
  'server.details.autoStart': 'Démarrage automatique',
  'server.details.restartOnCrash': 'Redémarrer après un plantage',
  'server.details.ram': 'Mémoire min. / max.',
  'server.details.ramValue': '{min} Mo / {max} Mo',
  'server.details.folder': 'Dossier',
  'server.details.modpack': 'Créé à partir du modpack',

  // InstancePerformance
  'server.performance.title': 'Surveillance du serveur',
  'server.performance.lead':
    'Utilisation de votre Lodestone Core par l’instance',
  'server.performance.cpuLoad': 'CHARGE CPU : {value} %',
  'server.performance.memoryUsed': 'MÉMOIRE UTILISÉE : {value} Gio',
  'server.performance.loadingGraph': 'Chargement du graphique',
  'server.performance.cpuUsage': 'Utilisation du CPU',
  'server.performance.memoryUsage': 'Utilisation de la mémoire',
  'server.performance.unitPercent': ' %',
  'server.performance.unitGiB': ' Gio',

  // PerformanceGraph
  'server.graph.seconds': '{seconds} s',
  'server.graph.tooltipTime': '{time} :',

  // InstancePlayerList
  'server.players.title': 'Liste des joueurs',
  'server.players.lead': 'Tous les joueurs actuellement en ligne',
  'server.players.sortByName': 'NOM',
  'server.players.avatarAlt': 'Avatar de {name}',
  'server.players.linkedUser':
    'L’utilisateur Lodestone qui a associé ce compte Minecraft',
  'server.players.none': 'Aucun joueur en ligne',

  // InstanceSettings
  'server.settings.pageTitle': 'Paramètres du serveur',
  'server.settings.automation': 'Automatisation',
  'server.settings.automationLead':
    'Quand Lodestone démarre ce serveur de lui-même.',
  'server.settings.autoStart': 'Démarrage automatique',
  'server.settings.autoStartHint':
    'Démarrer ce serveur au démarrage de Lodestone Core',
  'server.settings.restartOnCrash': 'Redémarrer après un plantage',
  'server.settings.restartOnCrashHint': 'Redémarrer ce serveur s’il plante',
  'server.settings.notAvailable': 'Indisponible pour le moment',
  'server.settings.notAvailableHint':
    'Essayez de démarrer ce serveur au moins une fois',
  'server.settings.noPermission': 'Pas d’autorisation',
  'server.settings.dangerZone': 'Zone dangereuse',
  'server.settings.dangerZoneLead':
    'Ces paramètres peuvent causer des dommages irréversibles à votre serveur.',
  'server.settings.deleteServer': 'Supprimer le serveur',
  'server.settings.deleteServerHint':
    'Supprime définitivement ce serveur et ses données',
  'server.settings.deleteTitle': 'Supprimer « {name} »',
  'server.settings.deleteConfirm': 'Je comprends, supprimer ce serveur',
  'server.settings.deleteCannotUndo': 'Cette action est irréversible.',
  'server.settings.deleteWarning':
    '{cannotUndo} Les paramètres, mondes et sauvegardes de cette instance seront définitivement supprimés. Sauvegardez vos données importantes avant de continuer.',

  // SettingField
  'server.settingField.unknownType': 'Erreur d’envoi : type de valeur inconnu.',

  // PublicListingSetting
  'server.publicListing.title': 'Partage',
  'server.publicListing.lead': 'Qui peut trouver ce serveur sans se connecter.',
  'server.publicListing.listedPublicly': 'Listé publiquement',
  'server.publicListing.public': 'Public',
  'server.publicListing.ownerOnly': 'Seul le propriétaire peut modifier ceci',
  'server.publicListing.flag.status': 'État et joueurs',
  'server.publicListing.flag.address': 'Adresse pour rejoindre',
  'server.publicListing.flag.clientPack': 'Téléchargement des mods client',
  'server.publicListing.shownText':
    'Sans se connecter, toute personne qui peut accéder à ce core voit {list} de ce serveur.',
  'server.publicListing.shown.nameVersion': 'le nom et la version',
  'server.publicListing.shown.status': 'l’état et le nombre de joueurs',
  'server.publicListing.shown.address': 'l’adresse',
  'server.publicListing.shown.clientPack': 'les mods client',
  'server.publicListing.turnOnFirst':
    'Activez d’abord « Lister les serveurs publiquement ».',
  'server.publicListing.hiddenWhileOff':
    '{shown} Masqué tant que « Lister les serveurs publiquement » est désactivé.',
  'server.publicListing.notShown': 'Non visible sans se connecter.',
  'server.publicListing.noServers':
    'Aucun serveur Minecraft Java Edition pour l’instant.',

  // ResourcePackSetting
  'server.resourcePack.title': 'Pack de ressources',
  'server.resourcePack.lead':
    'Un pack que les joueurs sont invités à télécharger en rejoignant le serveur.',
  'server.resourcePack.url': 'URL',
  'server.resourcePack.urlHint':
    'Un lien direct vers le zip du pack. Laissez vide pour aucun pack.',
  'server.resourcePack.sha1Hint':
    'Permet aux joueurs de vérifier le téléchargement et de mettre le pack en cache.',
  'server.resourcePack.sha1Error':
    'Le SHA-1 comporte 40 caractères hexadécimaux.',
  'server.resourcePack.compute': 'Calculer',
  'server.resourcePack.downloaded': '{size} téléchargés.',
  'server.resourcePack.required': 'Exiger le pack',
  'server.resourcePack.requiredHint':
    'Les joueurs qui le refusent sont déconnectés.',
  'server.resourcePack.prompt': 'Message',
  'server.resourcePack.promptHint':
    'Le message affiché quand le pack est proposé aux joueurs. Texte brut ou composant de texte JSON.',
  'server.resourcePack.remove': 'Retirer le pack',
  'server.resourcePack.saved': 'Pack de ressources enregistré',
  'server.resourcePack.savedRestart':
    'Pack de ressources enregistré. Redémarrez le serveur pour l’utiliser.',

  // InstanceCard
  'server.action.start': 'Démarrer',
  'server.action.stop': 'Arrêter',
  'server.action.restart': 'Redémarrer',
  'server.action.kill': 'Forcer l’arrêt',
  'server.card.power': 'Alimentation',
  'server.card.noScripting':
    'Ce core a été compilé sans prise en charge des scripts (Deno) : les serveurs atom ne sont pas disponibles.',
  'server.card.codeExecutionDisabled':
    'L’exécution de code est désactivée sur ce core',
  'server.card.copyIp': 'Copier l’IP',
  'server.card.online': '{players}/{max} en ligne',

  // InstanceLoadingPill
  'server.loading.settingUp': 'Configuration… ({percent})',

  // apis.ts
  'server.api.deletedFile': 'Fichier supprimé : {name}',
  'server.api.deletedDirectory': 'Dossier supprimé : {path}',
  'server.api.downloadFailed': 'Échec du téléchargement de {name} : {error}',
  'server.api.uploading': {
    one: 'Importation de {count} fichier',
    other: 'Importation de {count} fichiers',
  },
  'server.api.moveFailed': 'Échec du déplacement de {name} : {error}',
  'server.api.zipFailed': 'Échec du lancement de la compression : {error}',
  'server.api.zipping': {
    one: 'Compression de {count} élément…',
    other: 'Compression de {count} éléments…',
  },
  'server.api.unzipFailed': 'Échec de la décompression de {name} : {error}',
  'server.api.unzipping': 'Décompression de {name}…',
  'server.api.tooManyAttemptsSeconds': {
    one: 'Trop de tentatives de connexion échouées. Patientez {count} seconde, puis réessayez.',
    other:
      'Trop de tentatives de connexion échouées. Patientez {count} secondes, puis réessayez.',
  },
  'server.api.tooManyAttemptsMinutes': {
    one: 'Trop de tentatives de connexion échouées. Patientez {count} minute, puis réessayez.',
    other:
      'Trop de tentatives de connexion échouées. Patientez {count} minutes, puis réessayez.',
  },
  'server.api.tooManyAttempts':
    'Trop de tentatives de connexion échouées. Patientez quelques minutes, puis réessayez.',
  'server.api.loginFailed': 'Échec de la connexion : {message}',
  'server.api.loginError': 'Erreur : {message}',
} satisfies CatalogOf<typeof en>;
