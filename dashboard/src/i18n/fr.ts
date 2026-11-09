// French catalog. Same keys and placeholders as en.ts (checked by tsc and
// catalogs.test.ts).
//
// Style: standard French UI wording, vouvoiement, infinitives on buttons
// (« Enregistrer »), sentence case. A non-breaking space before : ; ! ? and
// inside « ». Product names (Lodestone, Lodestone Core, Minecraft, Modrinth,
// Playit) stay as they are. « Core » is kept for Lodestone Core (masculine).
//
// Glossary:
//   server (a game server)           serveur
//   core (Lodestone Core)            core (m.)
//   instance                         instance (f.)
//   dashboard                        tableau de bord
//   console                          console (f.)
//   settings                         paramètres
//   user / owner                     utilisateur / propriétaire
//   permission                       autorisation
//   sign in / sign out               se connecter / se déconnecter
//   password / username              mot de passe / nom d’utilisateur
//   API key / token                  clé d’API / jeton
//   file / folder                    fichier / dossier
//   upload / download                importer / télécharger
//   mod / modpack                    mod / modpack
//   resource pack                    pack de ressources
//   macro                            macro (f.)
//   player / world                   joueur / monde
//   port / address                   port / adresse
//   memory (RAM)                     mémoire
//   start / stop / restart / kill    démarrer / arrêter / redémarrer / forcer l’arrêt
//   running / stopped                en marche / arrêté
//   tunnel (Playit)                  tunnel
//   log / event                      journal / événement
//   save / delete / remove           enregistrer / supprimer / retirer
//   enable / disable                 activer / désactiver
//   connection / network             connexion / réseau
//   default / search                 par défaut / rechercher
//   public listing                   annonce publique
import type { Messages } from './types.ts';

export const fr: Messages = {
  // ===== Common words =====

  'common.cancel': 'Annuler',
  'common.confirm': 'Confirmer',
  'common.save': 'Enregistrer',
  'common.saved': 'Enregistré',
  'common.saving': 'Enregistrement…',
  'common.delete': 'Supprimer',
  'common.remove': 'Retirer',
  'common.rename': 'Renommer',
  'common.edit': 'Modifier',
  'common.create': 'Créer',
  'common.add': 'Ajouter',
  'common.close': 'Fermer',
  'common.back': 'Retour',
  'common.next': 'Suivant',
  'common.done': 'Terminé',
  'common.copy': 'Copier',
  'common.copied': 'Copié',
  'common.copiedToClipboard': 'Copié dans le presse-papiers',
  'common.retry': 'Réessayer',
  'common.refresh': 'Actualiser',
  'common.search': 'Rechercher',
  'common.loading': 'Chargement…',
  'common.yes': 'Oui',
  'common.no': 'Non',
  'common.ok': 'OK',
  'common.on': 'Activé',
  'common.off': 'Désactivé',
  'common.enabled': 'Activé',
  'common.disabled': 'Désactivé',
  'common.none': 'Aucun',
  'common.unknown': 'Inconnu',
  'common.default': 'Par défaut',
  'common.optional': 'Facultatif',
  'common.required': 'Obligatoire',
  'common.learnMore': 'En savoir plus',
  'common.showMore': 'Afficher plus',
  'common.showLess': 'Afficher moins',
  'common.you': 'Vous',
  'common.unknownError': 'Erreur inconnue',
  'common.networkError': 'Erreur réseau',
  'common.networkErrorWithMessage': 'Erreur réseau : {message}',
  'common.invalidToken':
    'Jeton non valide : déconnectez-vous, puis reconnectez-vous',
  'common.notAnInteger': 'Ce n’est pas un nombre entier',
  'common.notANumber': 'Ce n’est pas un nombre valide',
  'common.justNow': 'à l’instant',
  'common.pageTitle': '{page} - Lodestone',
  'common.state.Starting': 'Démarrage',
  'common.state.Running': 'En marche',
  'common.state.Stopping': 'Arrêt en cours',
  'common.state.Stopped': 'Arrêté',
  'common.state.Error': 'Erreur',
  // Form fields and settings rows (Atoms/Form, Atoms/Config)
  'common.fieldLabel': '{label} :',
  'common.select': 'Sélectionner…',
  'common.noPermission': 'Aucune autorisation',
  'common.input.cannotBeEmpty': 'Ne peut pas être vide',
  'common.input.mustBeNumber': 'Doit être un nombre',
  'common.input.mustBeGreaterThan': 'Doit être supérieur à {min}',
  'common.input.mustBeLessThan': 'Doit être inférieur à {max}',
  'common.combo.addCustom': 'Ajouter « {query} »',
  'common.combo.nothingFound': 'Aucun résultat.',
  // Table
  'common.table.noData': 'Aucune donnée disponible pour le moment',
  // ClipboardTextfield: text that copies itself when clicked
  'common.clipboard.copyToClipboard': 'Copier dans le presse-papiers',
  'common.clipboard.copiedText': '« {text} » copié',
  // The server resource pack URL check (utils/resourcePack.ts)
  'common.resourcePack.urlSpaces': 'L’URL ne peut pas contenir d’espaces.',
  'common.resourcePack.notAUrl': 'Ce n’est pas une URL.',
  'common.resourcePack.urlScheme':
    'L’URL doit commencer par http:// ou https://.',

  // ===== App shell: navigation, top bar, settings frame, sign-in and setup, notifications =====

  'shell.session.expired': 'Votre session a expiré, veuillez vous reconnecter',
  'shell.session.invalid':
    'Votre session n’est plus valide, veuillez vous reconnecter',
  'shell.token.invalid': 'Jeton non valide',
  'shell.token.invalidExp': 'exp non valide dans le jeton',
  'shell.token.invalidUid': 'uid non valide dans le jeton',
  'shell.token.expired': 'Jeton expiré',

  'shell.error.invalidStatusCode': 'Code d’état non valide',
  'shell.error.invalidResponse': 'Réponse non valide',

  'shell.settings.general': 'Général',
  'shell.settings.users': 'Utilisateurs',
  'shell.settings.profile': 'Profil',

  'shell.settings.pagesButton': 'Pages des paramètres',
  'shell.settings.close': 'Fermer les paramètres',
  'shell.settings.closeTooltip': 'Fermer (Échap)',

  'shell.settingsNav.label': 'Paramètres',
  'shell.settingsNav.core': 'Core',
  'shell.settingsNav.account': 'Compte',
  'shell.settingsNav.users': 'Utilisateurs',
  'shell.settingsNav.unselectUser': 'Désélectionner',

  'shell.beta.title': 'Merci d’utiliser la bêta de Lodestone !',
  'shell.beta.body':
    'Merci d’utiliser la bêta de Lodestone ! Si vous rencontrez un problème, signalez-le-nous sur notre serveur Discord ou sur Github.',
  'shell.versionMismatch.title': 'Mise à jour requise !',
  'shell.versionMismatch.continue':
    'J’ai compris, continuer sans mettre à jour',
  'shell.versionMismatch.coreVersion': 'Version du core :',
  'shell.versionMismatch.dashboardVersion': 'Version du tableau de bord :',
  'shell.versionMismatch.incompatible':
    'Votre tableau de bord et votre core sont incompatibles !',
  'shell.versionMismatch.body':
    'Cela peut provoquer un comportement inattendu. Veuillez mettre à jour votre core vers la dernière version. Consultez {link} pour en savoir plus.',
  'shell.versionMismatch.wiki': 'le wiki',
  'shell.versionMismatch.toast':
    'Versions différentes ! Votre core est-il à jour ? Core : {core}, tableau de bord : {dashboard}',
  'shell.localCore.title': 'Nouveau core local détecté',
  'shell.localCore.body':
    'Un core local qui n’est pas encore configuré a été détecté. Voulez-vous configurer {name} ?',
  'shell.localCore.setup': 'Configurer',
  'shell.localCore.skip': 'Ignorer',
  'shell.setupRequired.title': 'Configuration requise',
  'shell.setupRequired.body':
    '{name} n’est pas encore configuré. Veuillez terminer la configuration.',
  'shell.setupRequired.changeCore': 'Changer de core',
  'shell.coreError.title': 'Erreur de connexion au core',
  'shell.coreError.body':
    'Une erreur s’est produite lors de la connexion à {address}. Veuillez choisir un autre core, actualiser la page ou simplement attendre que le core soit de nouveau en ligne.',
  'shell.coreError.back': 'Retour au choix du core',
  'shell.coreError.continue': 'Continuer avec ce core',

  'shell.topNav.degraded':
    '{refresh} pour obtenir les dernières données sur Firefox.',
  'shell.topNav.refresh': 'Actualisez',
  'shell.topNav.devBuild': 'Version de développement locale',

  'shell.connection.connected': 'Connecté',
  'shell.connection.connectedDetail': 'Mises à jour en direct',
  'shell.connection.connecting': 'Connexion…',
  'shell.connection.connectingDetail': 'En attente de la réponse du core',
  'shell.connection.degraded': 'Dégradé',
  'shell.connection.degradedDetail':
    'Mises à jour en direct indisponibles, actualisez pour mettre à jour',
  'shell.connection.disconnected': 'Déconnecté',
  'shell.connection.disconnectedDetail': 'Impossible de joindre le core',
  'shell.connection.address': 'Adresse',
  'shell.connection.version': 'Version',
  'shell.connection.host': 'Hôte',
  'shell.connection.system': 'Système',
  'shell.connection.uptime': 'Disponibilité',
  'shell.connection.uptimeDays': '{days} j {hours} h',
  'shell.connection.uptimeHours': '{hours} h {minutes} min',
  'shell.connection.uptimeMinutes': '{minutes} min',
  'shell.connection.coreFallback': 'Core',
  'shell.connection.ariaLabel': '{core} : {status}',

  'shell.nav.allInstances': 'Toutes les instances',
  'shell.nav.integrations': 'Intégrations',
  'shell.nav.selectedInstance': 'Instance sélectionnée',
  'shell.nav.noSelection': 'Le serveur sélectionné apparaîtra ici',
  'shell.nav.loadingServers': 'Chargement des serveurs',
  'shell.nav.logInToView': 'Connectez-vous pour voir vos serveurs.',
  'shell.nav.newServer': 'Nouveau serveur…',
  'shell.nav.noPermission': 'Aucune autorisation',

  'shell.notifications.title': 'Notifications',
  'shell.notifications.inProgress': 'En cours',
  'shell.notifications.all': 'Toutes les notifications',
  'shell.notifications.loading': 'Chargement des notifications',
  'shell.notifications.empty': 'Aucune notification pour le moment !',

  'shell.event.state.Starting': 'Le serveur {name} démarre !',
  'shell.event.state.Running': 'Le serveur {name} a démarré !',
  'shell.event.state.Stopping': 'Le serveur {name} s’arrête !',
  'shell.event.state.Stopped': 'Le serveur {name} s’est arrêté !',
  'shell.event.state.Error': 'Le serveur {name} a rencontré une erreur !',
  'shell.event.warningToast':
    'Avertissement : un serveur a émis un avertissement. Cela ne devrait pas arriver, veuillez le signaler aux développeurs.',
  'shell.event.warning': 'Le serveur {name} a émis un avertissement',
  'shell.event.error': 'Le serveur {name} a rencontré une erreur',
  'shell.event.playersJoined': '{players} a rejoint {name}',
  'shell.event.playersLeft': '{players} a quitté {name}',
  'shell.event.playersJoinedAndLeft':
    '{joined} a rejoint {name} et {left} a quitté {name}',
  'shell.event.playerMessage': '{player} a dit {message} sur {name}',
  'shell.event.apiKeyCreated': 'Clé d’API « {name} » créée',
  'shell.event.apiKeyRevoked': 'Clé d’API « {name} » révoquée',
  'shell.event.macroStarted': 'Macro {pid} démarrée sur {server}',
  'shell.event.macroDetached': 'Macro {pid} détachée sur {server}',
  'shell.event.macroStopped':
    'Macro {pid} arrêtée sur {server} avec l’état {status}',
  'shell.event.playitStarted': 'Agent Playitgg démarré !',
  'shell.event.playitLoading': 'Chargement de l’agent Playitgg…',
  'shell.event.playitStopped': 'Agent Playitgg arrêté !',
  'shell.event.loadFailed': 'Impossible de charger les notifications : {error}',

  'shell.noScripting':
    'Ce core a été compilé sans prise en charge des scripts (Deno) : les macros ne sont pas disponibles.',

  'shell.userMenu.guest': 'Invité',
  'shell.userMenu.account': 'Compte : {name}',
  'shell.userMenu.signedInAs': 'Connecté en tant que {name}',
  'shell.userMenu.notSignedIn': 'Non connecté',
  'shell.userMenu.signOut': 'Se déconnecter',
  'shell.userMenu.signIn': 'Se connecter',
  'shell.userMenu.changeCore': 'Changer de core',
  'shell.userMenu.settings': 'Paramètres',

  'shell.userBox.changePassword': 'Changer le mot de passe',
  'shell.userBox.cantDeleteSelfTitle':
    'Vous ne pouvez pas vous supprimer vous-même',
  'shell.userBox.cantDeleteSelf':
    'Vous ne pouvez pas vous supprimer vous-même.',
  'shell.userBox.deleteTitle': 'Supprimer l’utilisateur',
  'shell.userBox.deleteConfirm':
    'Voulez-vous vraiment supprimer {name} ? Cette action est irréversible.',
  'shell.userBox.deleteUser': 'Supprimer l’utilisateur',
  'shell.userBox.admin': 'Admin',
  'shell.userBox.owner': 'Propriétaire',

  'shell.connectionHelp.trustCertificate':
    'Si le core est en marche et utilise son certificat intégré, ce navigateur doit faire confiance à ce certificat une fois. Ouvrez {link}, acceptez l’avertissement de certificat, puis revenez et réessayez.',
  'shell.connectionHelp.fingerprint':
    'Pour vous assurer qu’il s’agit bien de votre core, comparez l’empreinte du certificat affichée par votre navigateur avec celle du journal du core ou de ses paramètres.',
  'shell.connectionHelp.plainHttp':
    'Par défaut, les cores Lodestone refusent le HTTP simple venant d’autres appareils.',
  'shell.connectionHelp.savedCore':
    'Connectez un nouveau core avec la même adresse et le protocole https.',
  'shell.connectionHelp.changeProtocol':
    'Passez le protocole à https et réessayez.',

  'shell.login.checkFields':
    '{error} : veuillez vérifier que les champs sont correctement remplis.',
  'shell.login.invalidResponse':
    'Réponse non valide, la configuration est peut-être incorrecte',
  'shell.login.downloadCore': 'Télécharger Lodestone Core',
  'shell.login.continue': 'Continuer',
  'shell.login.or': 'OU',
  'shell.login.username': 'Nom d’utilisateur',
  'shell.login.password': 'Mot de passe',
  'shell.login.usernameRequired': 'Le nom d’utilisateur est obligatoire',
  'shell.login.passwordRequired': 'Le mot de passe est obligatoire',
  'shell.login.changeCore': 'Changer de core',
  'shell.login.signIn': 'Se connecter',

  'shell.login.withApiKey': 'Se connecter avec une clé d’API',
  'shell.login.withPassword':
    'Se connecter avec un nom d’utilisateur et un mot de passe',
  'shell.login.apiKey': 'Clé d’API',
  'shell.login.apiKeyHint':
    'Collez une clé qui commence par lsk_. Vous obtenez l’accès de la clé, et la révoquer vous déconnecte.',
  'shell.login.apiKeyRequired': 'Une clé d’API est requise',
  'shell.login.apiKeyFormat': 'Une clé d’API commence par lsk_',
  'shell.coreConnect.pageTitle': 'Connexion au core',
  'shell.coreConnect.title': 'Ajouter un core',
  'shell.coreConnect.httpErrorTitle': 'Erreur HTTP',
  'shell.coreConnect.mixedContent':
    'Autorisations du site manquantes. Autorisez le contenu mixte ou désactivez les extensions HTTPS du navigateur, puis actualisez la page. {link}',
  'shell.coreConnect.httpsWarning':
    'Vous devrez peut-être ajuster les paramètres de votre réseau et de votre navigateur. {link}',
  'shell.coreConnect.apiVersion': 'Version de l’API',
  'shell.coreConnect.protocol': 'Protocole',
  'shell.coreConnect.port': 'Port',
  'shell.coreConnect.address': 'Adresse IP/domaine',
  'shell.coreConnect.addressPlaceholder':
    'p. ex. 123.123.123.123 ou « myserver.com »',
  'shell.coreConnect.submit': 'Ajouter et continuer',

  'shell.coreSelect.pageTitle': 'Choisir un core',
  'shell.coreSelect.title': 'Choisir un Lodestone Core',
  'shell.coreSelect.existing': 'Choisir un core existant',
  'shell.coreSelect.connectNew': 'Connecter un core',

  'shell.coreSetup.pageTitle': 'Configurer un core',
  'shell.coreSetup.title': 'Créer le compte du propriétaire',
  'shell.coreSetup.findKey':
    'Consultez la sortie console du core pour trouver la « First time setup key »',
  'shell.coreSetup.setupKey': 'Clé de configuration',
  'shell.coreSetup.setupKeyPrefilled': 'Clé de configuration (préremplie)',
  'shell.coreSetup.confirmPassword': 'Confirmer le mot de passe',
  'shell.coreSetup.submit': 'Valider',
  'shell.coreSetup.passwordConfirmRequired':
    'La confirmation du mot de passe est obligatoire',
  'shell.coreSetup.passwordMismatch':
    'La confirmation doit correspondre au mot de passe',
  'shell.coreSetup.setupKeyRequired': 'La clé de configuration est obligatoire',
  'shell.coreSetup.failed':
    'Une erreur s’est produite lors de la configuration du core',

  'shell.coreConfig.pageTitle': 'Configurer le nouveau core',
  'shell.coreConfig.title': 'Personnaliser votre core',
  'shell.coreConfig.coreName': 'Nom du core',
  'shell.coreConfig.domain': 'Domaine/IP public (facultatif)',
  'shell.coreConfig.coreNameRequired': 'Le nom du core est obligatoire',
  'shell.coreConfig.domainTooLong':
    'Le domaine ne peut pas dépasser {max} caractères',

  'shell.welcome.pageTitle': 'Bienvenue dans Lodestone',
  'shell.welcome.wiki': 'Wiki et FAQ',
  'shell.welcome.title': 'Bienvenue dans Lodestone !',
  'shell.welcome.version': 'VERSION {version}',
  'shell.welcome.beta': 'Lodestone est un logiciel en bêta !',
  'shell.welcome.betaBody':
    'La prise en charge des navigateurs est limitée et des bugs sont à prévoir. La stabilité des fonctionnalités et la compatibilité future ne sont pas garanties. Vous pouvez consulter les problèmes connus et en signaler de nouveaux sur notre {github}.',
  'shell.welcome.connectExisting': 'Se connecter à un core existant',
  'shell.welcome.setupCore': 'Configurer Lodestone Core',

  'shell.userLogin.switchAccount': 'Changer de compte',
  'shell.userSelect.pageTitle': 'Choisir un utilisateur',
  'shell.userSelect.loadingUser': 'Chargement de l’utilisateur',
  'shell.userSelect.continueAs': 'Continuer en tant que {name}',
  'shell.userSelect.continueAsCurrent': 'Continuer avec l’utilisateur actuel',
  'shell.userSelect.switchUser': 'Changer d’utilisateur',

  'shell.home.pageTitle': 'Accueil',
  'shell.home.title': 'Gérez vos serveurs',
  'shell.home.subtitle':
    'Pour commencer, choisissez un serveur dans le menu ou créez-en un',
  'shell.home.create': 'Créer un serveur',

  'shell.notFound.title': 'Page introuvable',
  'shell.notFound.goHome': 'Retour à l’accueil',

  // ===== Settings: core settings, users, profile, API keys =====

  'settings.language.label': 'Langue',
  'settings.language.auto': 'Langue du navigateur ({language})',

  'settings.theme.label': 'Thème',
  'settings.theme.system': 'Système',
  'settings.theme.light': 'Clair',
  'settings.theme.dark': 'Sombre',
  'settings.theme.systemHint':
    'Suit le réglage clair ou sombre de votre appareil',
  'settings.theme.lightHint': 'Toujours clair',
  'settings.theme.darkHint': 'Toujours sombre',

  'settings.toc.label': 'Sections',

  'settings.legacy.title': 'Paramètres du core',

  'settings.core.documentTitle': 'Paramètres de Lodestone Core',
  'settings.core.title': 'Général',
  'settings.core.section.core': 'Core',
  'settings.core.section.public': 'Serveurs publics',
  'settings.core.section.publicLead': 'Visible sans connexion.',
  'settings.core.section.info': 'Informations',
  'settings.core.section.danger': 'Zone de danger',
  'settings.core.section.dangerLead': 'À modifier avec prudence.',
  'settings.core.name.label': 'Nom du core',
  'settings.core.name.empty': 'Le nom ne peut pas être vide',
  'settings.core.name.tooLong': 'Le nom ne peut pas dépasser {max} caractères',
  'settings.core.domain.label': 'Domaine ou IP publique',
  'settings.core.domain.tooLong':
    'Le domaine ne peut pas dépasser {max} caractères',
  'settings.core.domain.tooltip':
    'C’est votre IP publique si vous avez configuré une redirection de port. Sinon, laissez tel quel.',
  'settings.core.playit.label': 'Activer l’intégration Playit',
  'settings.core.playit.hint':
    'Ouvre les serveurs à Internet sans redirection de port.',
  'settings.core.listPublicly.label': 'Lister les serveurs publiquement',
  'settings.core.listPublicly.hint':
    'Toute personne qui peut joindre ce core voit les serveurs que vous rendez publics ci-dessous.',
  'settings.core.localClaim.label':
    'Autoriser les applications de cet ordinateur à obtenir l’accès propriétaire',
  'settings.core.localClaim.hint':
    'Lodestone Launcher peut devenir propriétaire en prouvant qu’il peut écrire dans le dossier de données de ce core. Tout ce qui peut y écrire peut déjà modifier chaque utilisateur et réglage\u00a0: cela ne compte que sur un ordinateur partagé.',
  'settings.core.safeMode.label': 'Mode sécurisé',
  'settings.core.safeMode.hint':
    'Réserve l’accès administrateur de cette machine au propriétaire.',
  'settings.core.safeMode.offTitle': 'Désactiver le mode sécurisé ?',
  'settings.core.safeMode.offBody':
    'Voulez-vous vraiment désactiver le mode sécurisé ? Vous pourrez alors permettre à d’autres utilisateurs que vous d’exécuter des commandes potentiellement dangereuses. Assurez-vous de faire confiance à tous les utilisateurs à qui vous donnez ces autorisations.',
  'settings.core.turnOff': 'Désactiver',
  'settings.core.codeExecution.label': 'Exécution de code',
  'settings.core.codeExecution.hint':
    'Macros, serveurs atom, extensions, scripts de serveur et commandes Java personnalisées.',
  'settings.core.codeExecution.hintJavaOnly': 'Commandes Java personnalisées.',
  'settings.core.codeExecution.locked':
    'Verrouillé par l’administrateur de ce core (LODESTONE_DISABLE_CODE_EXECUTION ou --disable-code-execution).',
  'settings.core.codeExecution.offTitle': 'Désactiver l’exécution de code ?',
  'settings.core.codeExecution.offBody':
    'Les macros et serveurs atom en cours seront arrêtés. Les macros, serveurs atom, extensions, scripts de serveur et commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement.',
  'settings.core.codeExecution.offBodyJavaOnly':
    'Les commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement.',
  'settings.core.lanHttp.label': 'Autoriser l’accès LAN en HTTP simple',
  'settings.core.lanHttp.hint':
    'Sans chiffrement. D’autres personnes sur votre réseau peuvent lire les mots de passe.',
  'settings.core.lanHttp.confirmTitle':
    'Autoriser le HTTP simple depuis d’autres appareils ?',
  'settings.core.lanHttp.confirmBody':
    'Les appareils de votre réseau pourront se connecter en HTTP simple. Les mots de passe et jetons de session envoyés ainsi traversent le réseau sans chiffrement : n’importe qui d’autre sur le réseau peut les lire et prendre le contrôle de comptes sur ce core. N’autorisez cela que si un appareil ne peut pas utiliser https://.',
  'settings.core.lanHttp.allow': 'Autoriser',
  'settings.core.lanHttp.warningOwner':
    'D’autres appareils peuvent se connecter à ce core en HTTP simple : les mots de passe et jetons de session peuvent donc traverser votre réseau sans chiffrement. Désactivez « {setting} » ci-dessous et connectez-vous plutôt en https://.',
  'settings.core.lanHttp.warningOther':
    'D’autres appareils peuvent se connecter à ce core en HTTP simple : les mots de passe et jetons de session peuvent donc traverser votre réseau sans chiffrement. Connectez-vous en https:// et demandez au propriétaire de désactiver l’accès en HTTP simple.',
  'settings.core.openPort.label': 'Ouvrir un port',
  'settings.core.openPort.hint':
    'Redirige un port sur votre routeur avec {upnp}.',
  'settings.core.openPort.noPermission': 'Aucune autorisation',
  'settings.core.openPort.button': 'Ouvrir un port',
  'settings.core.openPort.portLabel': 'Port à ouvrir',
  'settings.core.openPort.portNotNumber': 'Le port doit être un nombre',
  'settings.core.openPort.portRequired': 'Le port est requis',
  'settings.core.openPort.portRange':
    'Le port doit être compris entre 1 et 65535',
  'settings.core.openPort.opened': 'Port {port} ouvert',
  'settings.core.info.coreVersion': 'Version du core',
  'settings.core.info.dashboardVersion': 'Version du tableau de bord',
  'settings.core.info.os': 'Système d’exploitation',
  'settings.core.info.arch': 'Architecture',
  'settings.core.info.cpu': 'Processeur',
  'settings.core.info.cpuCores': 'Cœurs du processeur',
  'settings.core.info.totalRam': 'Mémoire totale',
  'settings.core.info.noRam':
    'Pas de mémoire ? Comment faites-vous tourner Lodestone ?',
  'settings.core.info.httpsCertificate': 'Certificat HTTPS',
  'settings.core.info.fingerprint': 'Empreinte du certificat (SHA-256)',
  'settings.core.tls.selfSigned': 'Certificat auto-signé intégré',
  'settings.core.tls.userProvided': 'Votre propre certificat (tls/cert.pem)',
  'settings.core.tls.none': 'Indisponible, HTTP simple uniquement',

  'settings.users.documentTitle': 'Paramètres des utilisateurs',
  'settings.users.title': 'Utilisateurs',
  'settings.users.description': 'Les comptes de ce core et leurs droits.',
  'settings.users.allUsers': 'Tous les utilisateurs',
  'settings.users.allMembers': 'Tous les membres',
  'settings.users.allMembersCount': 'Tous les membres ({count})',
  'settings.users.createUser': 'Nouvel utilisateur',
  'settings.users.createTitle': 'Créer un compte utilisateur',
  'settings.users.createLead': 'Commence sans aucune autorisation.',
  'settings.users.loadingUsers': 'Chargement des utilisateurs',
  'settings.users.loadingUser': 'Chargement de l’utilisateur',
  'settings.users.loadingServers': 'Chargement des serveurs',
  'settings.users.permissions': 'Autorisations',
  'settings.users.permissionsLead': 'Ce que {username} peut faire sur ce core.',
  'settings.users.unsafePermissions': 'Autorisations à risque',
  'settings.users.unsafePermissionsLead':
    'Désactivez le mode sécurisé pour les accorder à d’autres utilisateurs.',
  'settings.users.ownerOnly': 'Macros, extensions et serveurs atom',
  'settings.users.ownerOnlyHint':
    'Réservé au propriétaire. Les serveurs Docker utilisent les autorisations habituelles.',

  'settings.permission.can_create_instance.title': 'Créer des serveurs',
  'settings.permission.can_create_instance.user':
    'L’utilisateur peut créer de nouveaux serveurs.',
  'settings.permission.can_create_instance.key':
    'La clé peut créer de nouveaux serveurs.',
  'settings.permission.can_delete_instance.title': 'Supprimer des serveurs',
  'settings.permission.can_delete_instance.user':
    'L’utilisateur peut supprimer n’importe quel serveur.',
  'settings.permission.can_delete_instance.key':
    'La clé peut supprimer n’importe quel serveur.',
  'settings.permission.can_read_global_file.title': 'Lire les fichiers globaux',
  'settings.permission.can_read_global_file.user':
    'L’utilisateur peut lire n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute.',
  'settings.permission.can_read_global_file.key':
    'La clé peut lire n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute.',
  'settings.permission.can_view_instance.title': 'Voir les serveurs',
  'settings.permission.can_view_instance.user':
    'L’utilisateur peut voir ces serveurs.',
  'settings.permission.can_view_instance.key': 'La clé peut voir ces serveurs.',
  'settings.permission.can_start_instance.title': 'Démarrer les serveurs',
  'settings.permission.can_start_instance.user':
    'L’utilisateur peut démarrer ces serveurs.',
  'settings.permission.can_start_instance.key':
    'La clé peut démarrer ces serveurs.',
  'settings.permission.can_stop_instance.title': 'Arrêter les serveurs',
  'settings.permission.can_stop_instance.user':
    'L’utilisateur peut arrêter ces serveurs.',
  'settings.permission.can_stop_instance.key':
    'La clé peut arrêter ces serveurs.',
  'settings.permission.can_access_instance_console.title':
    'Accéder à la console des serveurs',
  'settings.permission.can_access_instance_console.user':
    'L’utilisateur peut lire la console de ces serveurs et y envoyer des commandes. Cela donne de fait un accès opérateur au serveur.',
  'settings.permission.can_access_instance_console.key':
    'La clé peut lire la console de ces serveurs et y envoyer des commandes. Cela donne de fait un accès opérateur au serveur.',
  'settings.permission.can_access_instance_setting.title':
    'Accéder aux paramètres des serveurs',
  'settings.permission.can_access_instance_setting.user':
    'L’utilisateur peut lire et modifier les paramètres de ces serveurs.',
  'settings.permission.can_access_instance_setting.key':
    'La clé peut lire et modifier les paramètres de ces serveurs.',
  'settings.permission.can_read_instance_resource.title':
    'Lire les ressources des serveurs',
  'settings.permission.can_read_instance_resource.user':
    'L’utilisateur peut lire les ressources (mondes, mods, etc.) de ces serveurs.',
  'settings.permission.can_read_instance_resource.key':
    'La clé peut lire les ressources (mondes, mods, etc.) de ces serveurs.',
  'settings.permission.can_read_instance_file.title':
    'Lire les fichiers des serveurs',
  'settings.permission.can_read_instance_file.user':
    'L’utilisateur peut lire les fichiers de ces serveurs.',
  'settings.permission.can_read_instance_file.key':
    'La clé peut lire les fichiers de ces serveurs.',
  'settings.permission.can_write_instance_resource.title':
    'Modifier les ressources des serveurs',
  'settings.permission.can_write_instance_resource.user':
    'L’utilisateur peut modifier les ressources (mondes, mods, etc.) de ces serveurs.',
  'settings.permission.can_write_instance_resource.key':
    'La clé peut modifier les ressources (mondes, mods, etc.) de ces serveurs.',
  'settings.permission.can_write_instance_file.title':
    'Modifier les fichiers des serveurs',
  'settings.permission.can_write_instance_file.user':
    'L’utilisateur peut modifier les fichiers de ces serveurs.',
  'settings.permission.can_write_instance_file.key':
    'La clé peut modifier les fichiers de ces serveurs.',
  'settings.permission.can_write_global_file.title':
    'Modifier les fichiers globaux',
  'settings.permission.can_write_global_file.user':
    'L’utilisateur peut modifier n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute.',
  'settings.permission.can_write_global_file.key':
    'La clé peut modifier n’importe quel fichier de l’ordinateur sur lequel Lodestone Core s’exécute.',
  'settings.permission.can_manage_permission.title': 'Gérer les autorisations',
  'settings.permission.can_manage_permission.user':
    'L’utilisateur peut gérer les autorisations des autres utilisateurs.',
  'settings.permission.can_manage_permission.key':
    'La clé peut gérer les autorisations des autres utilisateurs.',

  'settings.profile.documentTitle': 'Profil utilisateur',
  'settings.profile.title': 'Profil',
  'settings.profile.guest': 'Invité',
  'settings.profile.notLoggedIn':
    'Vous n’êtes pas connecté ! Connectez-vous pour accéder à cette page.',
  'settings.profile.account': 'Compte',
  'settings.profile.username': 'Nom d’utilisateur',
  'settings.profile.password': 'Mot de passe',
  'settings.profile.changePassword': 'Changer le mot de passe',
  'settings.profile.appearance': 'Apparence',
  'settings.profile.appearanceLead':
    'Enregistré dans ce navigateur, pas sur le core.',
  'settings.profile.coloredConsole': 'Console en couleur',

  'settings.form.username': 'Nom d’utilisateur',
  'settings.form.password': 'Mot de passe',
  'settings.form.oldPassword': 'Ancien mot de passe',
  'settings.form.confirmPassword': 'Confirmer le mot de passe',
  'settings.form.usernameRequired': 'Le nom d’utilisateur est requis',
  'settings.form.passwordRequired': 'Le mot de passe est requis',
  'settings.form.confirmRequired':
    'La confirmation du mot de passe est requise',
  'settings.form.passwordsMustMatch':
    'Les mots de passe doivent être identiques',
  'settings.form.changePassword': 'Changer le mot de passe',
  'settings.form.createUser': 'Créer l’utilisateur',

  'settings.minecraft.title': 'Comptes Minecraft',
  'settings.minecraft.ownLead': 'Liés depuis Lodestone Launcher.',
  'settings.minecraft.ownEmpty':
    'Aucun compte Minecraft n’est lié. Connectez-vous à ce core dans Lodestone Launcher pour en lier un.',
  'settings.minecraft.userLead':
    'Les comptes Minecraft que {username} a liés depuis Lodestone Launcher.',
  'settings.minecraft.userEmpty': '{username} n’a lié aucun compte Minecraft.',
  'settings.minecraft.tooOld':
    'Ce core est trop ancien pour lier des comptes Minecraft.',
  'settings.minecraft.removeTitle': 'Retirer le compte Minecraft',
  'settings.minecraft.removeBody':
    'Retirer le lien vers {name} ? Il pourra être lié à nouveau depuis Lodestone Launcher.',
  'settings.minecraft.removed': '{name} retiré',

  'settings.apiKeys.title': 'Clés d’API',
  'settings.apiKeys.ownLead':
    'Permettent à la CLI, aux scripts et aux agents d’agir en votre nom sans votre mot de passe.',
  'settings.apiKeys.ownEmpty': 'Vous n’avez aucune clé d’API.',
  'settings.apiKeys.userLead':
    'Clés créées par {username}. En révoquer une coupe l’accès de ce qui l’utilise.',
  'settings.apiKeys.userEmpty': '{username} n’a aucune clé d’API.',
  'settings.apiKeys.create': 'Créer une clé d’API',
  'settings.apiKeys.created': 'Clé d’API « {name} » créée',
  'settings.apiKeys.loading': 'Chargement des clés d’API',
  'settings.apiKeys.revoke': 'Révoquer',
  'settings.apiKeys.revokeTitle': 'Révoquer la clé d’API',
  'settings.apiKeys.revokeBody':
    'Révoquer « {name} » ({display}) ? Tout ce qui utilise cette clé perd l’accès immédiatement. Cette action est irréversible.',
  'settings.apiKeys.revoked': 'Clé d’API « {name} » révoquée',
  'settings.apiKeys.createdOn': 'Créée le {date}',
  'settings.apiKeys.expiresOn': 'Expire le {date}',
  'settings.apiKeys.expiredOn': 'Expirée le {date}',
  'settings.apiKeys.expiresNever': 'Expiration : jamais',
  'settings.apiKeys.lastUsed': 'Dernière utilisation : {time}',
  'settings.apiKeys.lastUsedNever': 'Dernière utilisation : jamais',
  'settings.apiKeys.scope.full': 'Accès complet',
  'settings.apiKeys.scope.launcher':
    'Launcher : voir, démarrer et arrêter les serveurs',
  'settings.apiKeys.scope.launcherConsole':
    'Launcher : voir, démarrer et arrêter les serveurs, lire et utiliser la console',
  'settings.apiKeys.scope.launcherConsoleRead':
    'Launcher : voir, démarrer et arrêter les serveurs, lire la console',
  'settings.apiKeys.scope.launcherConsoleWrite':
    'Launcher : voir, démarrer et arrêter les serveurs, envoyer des commandes à la console',
  'settings.apiKeys.scope.custom': 'Personnalisé : {servers}, {permissions}',
  'settings.apiKeys.scope.servers': {
    one: '{count} serveur',
    other: '{count} serveurs',
  },
  'settings.apiKeys.scope.permissions': {
    one: '{count} autorisation',
    other: '{count} autorisations',
  },
  'settings.apiKeys.passwordSessionOnly':
    'Les clés d’API ne peuvent être gérées que si vous vous connectez avec votre mot de passe, pas avec une clé d’API.',
  'settings.apiKeys.createLead':
    'Donnez à chaque clé une courte durée de validité et uniquement l’accès nécessaire.',
  'settings.apiKeys.name': 'Nom',
  'settings.apiKeys.nameRequired': 'Le nom est requis',
  'settings.apiKeys.nameTooLong':
    'Le nom doit comporter au plus {max} caractères',
  'settings.apiKeys.expiresIn': 'Expire dans',
  'settings.apiKeys.expiry.30d': '30 jours',
  'settings.apiKeys.expiry.90d': '90 jours',
  'settings.apiKeys.expiry.1y': '1 an',
  'settings.apiKeys.expiry.never': 'Jamais',
  'settings.apiKeys.neverWarning':
    'Une clé qui n’expire jamais fonctionne jusqu’à ce que vous la révoquiez. Choisissez une durée, sauf si vous en avez vraiment besoin.',
  'settings.apiKeys.access': 'Accès',
  'settings.apiKeys.access.full':
    'Accès complet (tout ce que vous pouvez faire)',
  'settings.apiKeys.access.custom': 'Personnalisé',
  'settings.apiKeys.chooseOne': 'Choisissez au moins une autorisation.',
  'settings.apiKeys.createKey': 'Créer la clé',
  'settings.apiKeys.onlyYours':
    'Une clé ne peut avoir que les autorisations que vous avez vous-même. Les autorisations et les serveurs que vous n’avez pas sont grisés.',
  'settings.apiKeys.notYours': 'Vous n’avez pas cette autorisation.',
  'settings.apiKeys.noServers': 'Vous ne voyez aucun serveur.',
  'settings.apiKeys.unsafe': 'Paramètres à risque',
  'settings.apiKeys.unsafeSafeMode':
    'Le mode sécurisé est activé : le core refuse donc ces autorisations aux clés à accès personnalisé tant que le mode sécurisé n’est pas désactivé dans les paramètres du core.',
  'settings.apiKeys.ownerOnly': 'Macros, extensions et serveurs atom',
  'settings.apiKeys.ownerOnlyHint':
    'Seules les clés à accès complet peuvent utiliser les macros, les extensions et les serveurs atom.',
  'settings.apiKeys.newTitle': 'Clé d’API « {name} » créée',
  'settings.apiKeys.newCopyNow':
    'Copiez cette clé maintenant. Vous ne pourrez plus la voir.',
  'settings.apiKeys.newUseWithCli': 'Utilisez-la avec la CLI Lodestone :',
  'settings.apiKeys.newStore':
    'Conservez la clé comme un mot de passe. Toute personne qui la possède peut utiliser Lodestone avec ses accès jusqu’à son expiration ou sa révocation.',
  'settings.apiKeys.keyCopied': 'Clé d’API copiée dans le presse-papiers',
  'settings.apiKeys.cliCopied': 'Exemple CLI copié dans le presse-papiers',
  'settings.apiKeys.copyFailed':
    'Impossible de copier dans le presse-papiers. Sélectionnez le texte et copiez-le manuellement.',

  // ===== Server pages: tabs, overview, settings, performance, players =====

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

  // ===== Console, macros and Playit =====

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

  // ===== Creating a server =====

  // InstanceCreateForm / ModpackCreateForm: the steps in the dialog's sidebar
  'create.step.selectGame': 'Choisir un jeu',
  'create.step.basicSettings': 'Paramètres de base',
  'create.step.serverSettings': 'Paramètres du serveur',
  'create.step.autoSettings': 'Automatisation',
  'create.step.selectModpack': 'Choisir un modpack',
  'create.step.selectVersion': 'Choisir une version',

  // InstanceCreateForm: the pages' subtitles, buttons and errors
  'create.basicSettings.description':
    'Les paramètres de base de votre serveur.',
  'create.serverSettings.description': 'Configurez votre serveur.',
  'create.autoSettings.description':
    'Configurez automatiquement votre serveur.',
  'create.checkFields': 'Vérifiez que les champs sont correctement remplis',
  'create.createServer': 'Créer le serveur',
  'create.createFailed': 'Erreur lors de la création du serveur : {error}',
  'create.unknownValueType':
    'Erreur lors de la génération du formulaire : type de valeur inconnu.',

  // form.ts and ModpackCreateForm: the dashboard's own setup fields
  'create.field.name': 'Nom',
  'create.field.nameDescription': 'Le nom du serveur',
  'create.field.nameDefault': 'Mon serveur',
  'create.field.description': 'Description',
  'create.field.descriptionDescription': 'La description du serveur',
  'create.field.port': 'Port',
  'create.field.portDescription': 'Le port sur lequel exécuter le serveur',
  'create.field.minRam': 'Mémoire minimale',
  'create.field.minRamDescription':
    'La quantité minimale de mémoire à allouer au serveur, en Mo',
  'create.field.maxRam': 'Mémoire maximale',
  'create.field.maxRamDescription':
    'La quantité maximale de mémoire à allouer au serveur, en Mo',
  'create.field.autoStart': 'Démarrage automatique',
  'create.field.autoStartDescription':
    'Le serveur démarrera automatiquement au lancement de l’application',
  'create.field.restartOnCrash': 'Redémarrer en cas de plantage',
  'create.field.restartOnCrashDescription':
    'Le serveur redémarrera automatiquement s’il plante',

  // form.ts: validation of a setting; {name} is the setting's name (often sent by the core, untranslated)
  'create.validation.required': 'Le champ {name} est obligatoire',
  'create.validation.number': 'Le champ {name} doit être un nombre',
  'create.validation.integer': 'Le champ {name} doit être un entier',
  'create.validation.positive': 'Le champ {name} doit être positif',
  'create.validation.min':
    'Le champ {name} doit être supérieur ou égal à {min}',
  'create.validation.max':
    'Le champ {name} doit être inférieur ou égal à {max}',
  'create.validation.oneOf':
    'Le champ {name} doit correspondre à l’une des options proposées',

  // FormComboField: the options list of a field
  'create.combo.add': 'Ajouter « {query} »',
  'create.combo.nothingFound': 'Aucun résultat.',

  // GameTypeSelectForm: the first step, picking a game
  'create.gameType.title': 'Choisissez un jeu',
  'create.gameType.subtitle': 'À quoi servira votre serveur ?',
  'create.gameType.new': 'Nouveau',
  'create.gameType.modpack': 'Modpack Modrinth',
  'create.gameType.modpackDescription':
    'Créer un serveur Fabric à partir d’un modpack Modrinth',

  // GameTypeMappings: games' titles and descriptions
  'create.game.genericTitle': '{name} (générique)',
  'create.game.vanillaDescription':
    'Serveur Minecraft vanilla standard de Mojang.',
  'create.game.forgeDescription':
    'Framework de modding pour installer des mods et personnaliser votre expérience Minecraft.',
  'create.game.fabricDescription':
    'Chaîne d’outils de modding légère pour Minecraft.',
  'create.game.paperDescription':
    'Fork de Spigot hautes performances qui vise à corriger les incohérences de gameplay et de mécaniques.',
  'create.game.spigotDescription':
    'Logiciel de serveur Minecraft modifié, compatible avec les plugins, aux performances et options de personnalisation améliorées.',
  'create.game.otherDescription': 'Variante de Minecraft inconnue : {name}',
  'create.game.bedrockDescription': 'Serveur Minecraft Bedrock Edition.',
  'create.game.genericDescription': 'Jeu inconnu : {name}',

  // CreateInstanceFlow: read by screen readers while the dialog loads
  'create.loadingForm': 'Chargement du formulaire de création',

  // ModpackCreateForm: creating a server from a Modrinth modpack
  'create.modpack.sourceTitle': 'Choisissez un modpack',
  'create.modpack.sourceDescription':
    'Recherchez sur Modrinth, collez un lien Modrinth ou importez un fichier .mrpack. Seuls les modpacks Fabric sont pris en charge.',
  'create.modpack.searchPlaceholder':
    'Rechercher des modpacks ou coller un lien Modrinth',
  'create.modpack.onlyFabric': 'Afficher uniquement les modpacks Fabric',
  'create.modpack.upload': 'Importer un .mrpack',
  'create.modpack.versionTitle': 'Choisissez une version',
  'create.modpack.versionUploaded': 'Le modpack importé.',
  'create.modpack.versionDescription':
    'Choisissez la version du modpack à installer.',
  'create.modpack.downloading':
    'Téléchargement du fichier du modpack pour le lire…',
  'create.modpack.downloadingSize':
    'Téléchargement du fichier du modpack ({size}) pour le lire…',
  'create.modpack.versionHint':
    'Choisissez une version pour voir son contenu. Lodestone télécharge le fichier du modpack pour le lire ; rien n’est installé tant que vous n’avez pas cliqué sur Créer le serveur.',
  'create.modpack.settingsTitle': 'Paramètres du serveur',
  'create.modpack.loadingModpack': 'Chargement du modpack',
  'create.modpack.loadingVersions': 'Chargement des versions',
  'create.modpack.versionsFailed':
    'Impossible de charger les versions : {error}',
  'create.modpack.noVersions': 'Ce modpack n’a aucune version.',
  'create.modpack.select': 'Choisir',
  'create.modpack.selected': 'Choisie',
  'create.modpack.showMore': {
    one: 'Afficher plus ({count} restante)',
    other: 'Afficher plus ({count} restantes)',
  },
  // the summary of the picked version
  'create.modpack.summary.modpack': 'Modpack',
  'create.modpack.summary.loader': 'Chargeur',
  'create.modpack.summary.serverFiles': 'Fichiers du serveur',
  'create.modpack.summary.serverFilesValue': '{count} ({size} à télécharger)',
  'create.modpack.unsupported':
    'Ce modpack ne peut pas être utilisé : {reason}',
  // toasts and errors
  'create.modpack.inspectFailed': 'Impossible de lire le modpack : {error}',
  'create.modpack.notAModpack': 'Ce projet est de type {type}, pas un modpack',
  'create.modpack.chooseMrpack': 'Choisissez un fichier .mrpack',
  'create.modpack.noUploadId':
    'Le core n’a renvoyé aucun identifiant d’importation',
  'create.modpack.uploadFailed': 'Impossible d’importer le modpack : {error}',
  // validation of the settings step
  'create.modpack.nameRequired': 'Le nom est obligatoire',
  'create.modpack.portNumber': 'Le port doit être un nombre',
  'create.modpack.portInteger': 'Le port doit être un entier',
  'create.modpack.portRange': 'Le port doit être compris entre 1 et 65535',
  'create.modpack.portRequired': 'Le port est obligatoire',
  'create.modpack.minRamNumber': 'La mémoire minimale doit être un nombre',
  'create.modpack.minRamInteger': 'La mémoire minimale doit être un entier',
  'create.modpack.minRamMin': 'La mémoire minimale doit être d’au moins 1 Mo',
  'create.modpack.minRamRequired': 'La mémoire minimale est obligatoire',
  'create.modpack.maxRamNumber': 'La mémoire maximale doit être un nombre',
  'create.modpack.maxRamInteger': 'La mémoire maximale doit être un entier',
  'create.modpack.maxRamMin':
    'La mémoire maximale doit être au moins égale à la mémoire minimale',
  'create.modpack.maxRamRequired': 'La mémoire maximale est obligatoire',

  // ===== File manager =====

  // FileViewer: the page, its toolbar and dialogs
  'files.pageTitle': 'Explorateur de fichiers du serveur',
  'files.dropToUpload': 'Relâchez pour importer le fichier',
  'files.toolbar.addRemove': 'Ajouter/Retirer',
  'files.toolbar.selectAll': 'Tout sélectionner',
  'files.toolbar.deselectAll': 'Tout désélectionner',
  'files.toolbar.cutSelected': 'Couper la sélection',
  'files.toolbar.downloadSelected': 'Télécharger la sélection',
  'files.toolbar.unarchiveSelected': 'Extraire la sélection',
  'files.toolbar.deleteSelected': 'Supprimer la sélection',
  'files.toolbar.paste': {
    one: 'Coller {count} fichier',
    other: 'Coller {count} fichiers',
  },
  'files.toolbar.discardChanges': 'Annuler les modifications',
  'files.toolbar.upload': 'Importer',
  'files.cutToClipboard': {
    one: '{count} fichier coupé dans le presse-papiers',
    other: '{count} fichiers coupés dans le presse-papiers',
  },
  'files.zippingForDownload': 'Compression du dossier pour le téléchargement…',
  'files.downloadFailed': 'Échec du téléchargement de {name} : {error}',
  'files.onlyFilesUnzip': 'Seuls les fichiers peuvent être extraits',
  'files.onlyFilesUpload': 'Seuls des fichiers peuvent être importés',
  'files.delete.title': {
    one: 'Supprimer le fichier',
    other: 'Supprimer {count} fichiers',
  },
  'files.delete.message':
    'Voulez-vous vraiment supprimer les éléments suivants ?',
  'files.delete.directory': '(dossier)',
  'files.delete.unknownType': '(type inconnu)',
  'files.loadingFile': 'Chargement du fichier',
  'files.editor.title': 'Éditeur de fichiers',
  'files.editor.selectFile':
    'Sélectionnez un fichier pour afficher son contenu',
  'files.noReadPermission': 'Vous n’avez pas l’autorisation de lire ce dossier',
  'files.status.selected': {
    one: '{count} élément sélectionné',
    other: '{count} éléments sélectionnés',
  },
  'files.status.inClipboard': {
    one: '{count} élément dans le presse-papiers',
    other: '{count} éléments dans le presse-papiers',
  },

  // FileList: the list of files and folders
  'files.list.loading': 'Chargement des fichiers',
  'files.list.empty': 'Aucun fichier ici…',
  'files.list.unknownTime': 'Date de création inconnue',
  'files.list.folderIcon': 'icône de dossier',
  'files.list.fileIcon': 'icône de fichier',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': 'Couper',
  'files.contextMenu.paste': 'Coller',
  'files.contextMenu.pasteInto': 'Coller dans {name}',
  'files.contextMenu.zip': 'Compresser',
  'files.contextMenu.zipTo': 'Compresser en {archive}',
  'files.contextMenu.zipFile': 'Compresser {name} en {archive}',
  'files.contextMenu.zipFiles': {
    one: 'Compresser {count} fichier en {archive}',
    other: 'Compresser {count} fichiers en {archive}',
  },
  'files.contextMenu.unzipHere': 'Extraire ici',
  'files.contextMenu.unzipHereSmart': 'Extraire ici (intelligent)',
  'files.contextMenu.unzipTo': 'Extraire dans {name}',
  'files.contextMenu.copiedToClipboard': {
    one: '{count} élément copié dans le presse-papiers',
    other: '{count} éléments copiés dans le presse-papiers',
  },
  'files.contextMenu.cutToClipboard': {
    one: '{count} élément coupé dans le presse-papiers',
    other: '{count} éléments coupés dans le presse-papiers',
  },
  'files.contextMenu.unsupportedZip': 'Type d’archive non pris en charge',
  'files.newFolder': 'Nouveau dossier',
  'files.newFile': 'Nouveau fichier',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': 'Nommez votre fichier',
  'files.createFile.placeholder': 'Sans titre',
  'files.createFile.submit': 'Créer le fichier',
  'files.createFolder.label': 'Nommez votre dossier',
  'files.createFolder.placeholder': 'Dossier sans titre',
  'files.createFolder.submit': 'Créer le dossier',
  'files.rename.label': 'Nommez votre fichier',
  'files.rename.submit': 'Renommer le fichier',

  // FileEditor
  'files.editor.loading': 'Chargement de l’éditeur',

  // ===== Mods =====

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
};
