import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // GameConsole: a server's console tab
  'console.pageTitle': 'Console du serveur',
  'console.searchPlaceholder': 'Rechercher dans la console…',
  'console.regex': 'Regex',
  'console.status.noPermission': 'Aucune autorisation d’accès à la console',
  'console.status.loading': 'Chargement de la console…',
  'console.status.buffered':
    'Historique des messages. Pas de mises à jour en direct',
  'console.status.live': 'Console en direct',
  'console.status.liveNoBuffer':
    'Console en direct, mais impossible de récupérer l’historique. Votre connexion Internet est peut-être instable',
  'console.status.closed': 'La console est fermée',
  'console.status.error': 'Connexion perdue ou erreur',
  'console.serverStopped': 'Le serveur est arrêté',
  'console.input.noPermission': 'Aucune autorisation',
  'console.input.placeholder': 'Saisir une commande…',
  'console.noPermission':
    'Vous n’avez pas l’autorisation d’accéder à cette console',
  'console.loading': 'Chargement de la console',
  'console.empty': 'Aucun message dans la console pour l’instant',
  'console.hiddenLines': {
    one: '{count} ligne plus ancienne masquée (affichage des {max} plus récentes)',
    other:
      '{count} lignes plus anciennes masquées (affichage des {max} plus récentes)',
  },
  'console.sendFailed': 'Impossible d’envoyer la commande : {error}',

  // ConsoleLine: one line of console output
  'console.line.linkedUser':
    'L’utilisateur Lodestone qui a associé ce compte Minecraft',

  // CodeExecution: owner-only features and the code execution switch
  'console.ownerOnlyAtom':
    'Seul le propriétaire peut contrôler les serveurs atom',
  'console.ownerOnlyAtomSentence':
    'Seul le propriétaire peut contrôler les serveurs atom.',
  'console.commandLine.readOnly':
    'Lecture seule tant que l’exécution de code est désactivée sur ce core.',
  'console.commandLine.ownerOnly':
    'Seul le propriétaire peut modifier ce paramètre.',

  // Macros page
  'console.macros.pageTitle': 'Macros du serveur',
  'console.macros.intro': 'Toutes les macros de votre serveur',
  'console.macros.ownerOnly':
    'Les macros sont réservées au propriétaire de ce core.',
  'console.macros.codeExecutionDisabledBanner':
    'L’exécution de code est désactivée sur ce core. Les macros ne peuvent pas être exécutées ni configurées tant qu’elle n’est pas réactivée dans les paramètres du core.',
  'console.macros.codeExecutionDisabled':
    'L’exécution de code est désactivée sur ce core',
  'console.macros.tab.macros': 'Toutes les macros',
  'console.macros.tab.tasks': 'Tâches en cours',
  'console.macros.tab.history': 'Historique',
  'console.macros.column.name': 'NOM DE LA MACRO',
  'console.macros.column.lastRun': 'DERNIÈRE EXÉCUTION',
  'console.macros.column.created': 'CRÉÉE',
  'console.macros.column.finished': 'TERMINÉE',
  'console.macros.column.pid': 'ID DE PROCESSUS',
  'console.macros.noTime': 'N/D',
  'console.macros.run': 'Exécuter la macro',
  'console.macros.editConfig': 'Modifier la configuration',
  'console.macros.killTask': 'Forcer l’arrêt de la tâche',
  'console.macros.configTitle': 'Configuration de la macro',
  'console.macros.configRequired':
    'Cette macro nécessite une configuration supplémentaire.',
  'console.macros.noServer': 'Aucun serveur sélectionné',
  'console.macros.loadFailed': 'Impossible de charger les macros : {error}',
  'console.macros.openConfigFailed':
    'Erreur à l’ouverture de la configuration de la macro : {error}',
  'console.macros.runFailed':
    'Erreur lors de l’exécution de la macro : {error}',
  'console.macros.killFailed':
    'Impossible de forcer l’arrêt de la tâche : {error}',
  'console.macros.saveConfigFailed':
    'Impossible d’enregistrer la configuration de la macro : {error}',
  'console.macros.invalidState': 'Erreur : état non valide',

  // MacroCreateForm (not used yet)
  'console.macros.create.name': 'Nom de la macro',
  'console.macros.create.args': 'Arguments',
  'console.macros.create.submit': 'Créer la macro',
  'console.macros.create.nameRequired': 'Le nom de la macro est obligatoire',

  // PlayitggOverview: the Playit integration page
  'console.playit.loading': 'Chargement de playit.gg',
  'console.playit.loadFailed':
    'Impossible de charger l’état de playit.gg : {error}',
  'console.playit.signupFailed':
    'Impossible de générer un lien d’inscription : {error}',
  'console.playit.verifyFailed':
    'Impossible de vérifier le compte playit.gg : {error}',
  'console.playit.startFailed':
    'Impossible de démarrer l’agent playit.gg : {error}',
  'console.playit.stopFailed':
    'Impossible d’arrêter l’agent playit.gg : {error}',
  'console.playit.setupTitle': 'Configurer Playitgg',
  'console.playit.setupIntro':
    'Vous devez vous inscrire à Playitgg avant de pouvoir utiliser cette fonctionnalité. Générez un lien d’inscription avec le bouton ci-dessous !',
  'console.playit.generateLink': 'Générer le lien',
  'console.playit.setupFollowLink':
    'Suivez le lien ci-dessous et créez un compte ! Après avoir créé le compte et ouvert le tableau de bord, appuyez sur Vérifier ci-dessous.',
  'console.playit.verify': 'Vérifier',
  'console.playit.runnerTitle': 'Agent Playitgg',
  'console.playit.runner.Stopped': 'Arrêté',
  'console.playit.runner.Loading': 'Chargement',
  'console.playit.runner.Started': 'Démarré',
  'console.playit.trouble':
    'En cas de problème, vérifiez que votre tunnel est correctement configuré sur le site et que {running} !',
  'console.playit.troubleRunning': 'votre serveur est bien en marche',
  'console.playit.signupRequired':
    'Vous devez vous inscrire à Playitgg avant de pouvoir utiliser cette fonction.',
  'console.playit.signupLink': 'Cliquez ici pour vous inscrire.',
  'console.playit.tunnels': 'Tunnels',
  'console.playit.refreshTunnels': 'Actualiser les tunnels',
  'console.playit.tunnelsIntro':
    'Vos tunnels sont listés ci-dessous. Vous pouvez en créer d’autres et modifier leurs paramètres dans votre tableau de bord sur le site {link}.',
  'console.playit.loadingTunnels': 'Chargement des tunnels',
  'console.playit.localServer': 'Serveur local à l’adresse {address}',
  'console.playit.copyAddress': 'Copier l’adresse',
  'console.playit.noTunnels': 'Vous n’avez encore créé aucun tunnel',
  'console.playit.notSignedUp': 'Vous n’êtes pas encore inscrit',
  'console.playit.help': 'Besoin d’aide ?',
} satisfies CatalogOf<typeof en>;
