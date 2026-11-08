import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
} satisfies CatalogOf<typeof en>;
