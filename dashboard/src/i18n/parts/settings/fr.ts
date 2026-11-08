import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': 'Langue',
  'settings.language.hint':
    'La langue du tableau de bord. Les noms, journaux et messages des serveurs et du core restent tels quels.',
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
  'settings.core.description': 'Ces paramètres concernent le core lui-même.',
  'settings.core.section.core': 'Core',
  'settings.core.section.public': 'Serveurs publics',
  'settings.core.section.publicLead':
    'Ce que tout le monde peut voir sans se connecter. Vous seul pouvez le modifier.',
  'settings.core.section.info': 'Informations',
  'settings.core.section.infoLead':
    'Informations sur votre core et votre tableau de bord.',
  'settings.core.section.danger': 'Zone de danger',
  'settings.core.section.dangerLead':
    'Ces paramètres peuvent causer des dommages irréversibles à votre core.',
  'settings.core.name.label': 'Nom du core',
  'settings.core.name.hint':
    'Un surnom pour ce core. C’est ce que vous et les autres verrez en vous connectant à ce core',
  'settings.core.name.empty': 'Le nom ne peut pas être vide',
  'settings.core.name.tooLong': 'Le nom ne peut pas dépasser {max} caractères',
  'settings.core.domain.label': 'Domaine ou IP publique',
  'settings.core.domain.hint': 'Le domaine ou l’adresse IP publique de ce core',
  'settings.core.domain.tooLong':
    'Le domaine ne peut pas dépasser {max} caractères',
  'settings.core.domain.tooltip':
    'C’est votre IP publique si vous avez configuré une redirection de port. Sinon, laissez tel quel.',
  'settings.core.playit.label': 'Activer l’intégration Playit',
  'settings.core.playit.hint':
    'Active l’intégration Playit, qui permet aux utilisateurs d’ouvrir leurs serveurs au public sans redirection de port',
  'settings.core.listPublicly.label': 'Lister les serveurs publiquement',
  'settings.core.listPublicly.hint':
    'Permet à quiconque peut joindre ce core de voir les serveurs que vous rendez publics ci-dessous, sans se connecter, et uniquement ce que vous choisissez pour chacun. Désactiver cette option les masque tous d’un coup. Lodestone Launcher s’en sert pour les rejoindre.',
  'settings.core.safeMode.label': 'Mode sécurisé',
  'settings.core.safeMode.hint':
    'Tente d’empêcher les utilisateurs autres que le propriétaire d’accéder aux autorisations d’administration de votre machine',
  'settings.core.safeMode.offTitle': 'Désactiver le mode sécurisé ?',
  'settings.core.safeMode.offBody':
    'Voulez-vous vraiment désactiver le mode sécurisé ? Vous pourrez alors permettre à d’autres utilisateurs que vous d’exécuter des commandes potentiellement dangereuses. Assurez-vous de faire confiance à tous les utilisateurs à qui vous donnez ces autorisations.',
  'settings.core.turnOff': 'Désactiver',
  'settings.core.codeExecution.label': 'Exécution de code',
  'settings.core.codeExecution.hint':
    'Autorise les macros, les serveurs atom, les extensions, les scripts de serveur et les commandes Java personnalisées à s’exécuter sur ce core.',
  'settings.core.codeExecution.hintJavaOnly':
    'Autorise les commandes Java personnalisées à s’exécuter sur ce core.',
  'settings.core.codeExecution.locked':
    'Verrouillé par l’administrateur de ce core (LODESTONE_DISABLE_CODE_EXECUTION ou --disable-code-execution).',
  'settings.core.codeExecution.offTitle': 'Désactiver l’exécution de code ?',
  'settings.core.codeExecution.offBody':
    'Les macros et serveurs atom en cours seront arrêtés. Les macros, serveurs atom, extensions, scripts de serveur et commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement.',
  'settings.core.codeExecution.offBodyJavaOnly':
    'Les commandes Java personnalisées ne s’exécuteront plus sur ce core tant que vous n’aurez pas réactivé l’exécution de code. Rien n’est redémarré automatiquement.',
  'settings.core.lanHttp.label': 'Autoriser l’accès LAN en HTTP simple',
  'settings.core.lanHttp.hint':
    'Permet aux autres appareils de se connecter en http:// au lieu de https://. Les mots de passe et jetons de session traversent alors votre réseau sans chiffrement. Cette machine peut toujours utiliser http://.',
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
    'Tente une redirection de port sur votre routeur avec {upnp}. Les personnes sur Internet pourront ainsi se connecter à votre serveur.',
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
  'settings.users.description':
    'Les comptes de ce core et ce que chacun peut faire.',
  'settings.users.allUsers': 'Tous les utilisateurs',
  'settings.users.allMembers': 'Tous les membres',
  'settings.users.allMembersCount': 'Tous les membres ({count})',
  'settings.users.allMembersLead':
    'La liste de tous les utilisateurs. Cliquez sur un utilisateur pour le gérer.',
  'settings.users.createUser': 'Nouvel utilisateur',
  'settings.users.createTitle': 'Créer un compte utilisateur',
  'settings.users.createLead':
    'Cet utilisateur commencera sans aucune autorisation. Vous pourrez lui en accorder plus tard.',
  'settings.users.loadingUsers': 'Chargement des utilisateurs',
  'settings.users.loadingUser': 'Chargement de l’utilisateur',
  'settings.users.loadingServers': 'Chargement des serveurs',
  'settings.users.permissions': 'Autorisations',
  'settings.users.permissionsLead': 'Ce que {username} peut faire sur ce core.',
  'settings.users.unsafePermissions': 'Autorisations à risque',
  'settings.users.unsafePermissionsLead':
    'Désactivez le mode sécurisé dans les paramètres du core pour accorder ces autorisations aux utilisateurs autres que le propriétaire.',
  'settings.users.ownerOnly': 'Macros, extensions et serveurs atom',
  'settings.users.ownerOnlyHint':
    'Seul le propriétaire peut utiliser les macros, installer des extensions et créer ou contrôler des serveurs atom. Ces droits ne peuvent pas être accordés à d’autres utilisateurs. Les serveurs Docker utilisent les autorisations de serveur normales.',

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
  'settings.profile.description': 'Préférences du profil de {username}',
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
  'settings.profile.coloredConsoleHint':
    'Affiche les couleurs et les styles que les serveurs écrivent dans la console du serveur',

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
  'settings.minecraft.ownLead':
    'Les comptes que vous avez liés depuis Lodestone Launcher, qui vérifie auprès de Microsoft qu’ils vous appartiennent. Le tableau de bord affiche votre nom à côté.',
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
    'Les clés permettent à la CLI, aux scripts et aux agents d’utiliser Lodestone en votre nom, sans votre mot de passe. Révoquer une clé ne vous déconnecte nulle part ailleurs.',
  'settings.apiKeys.ownEmpty': 'Vous n’avez aucune clé d’API.',
  'settings.apiKeys.userLead':
    'Les clés créées par {username}. Révoquez une clé pour couper immédiatement l’accès de ce qui l’utilise. Seul {username} peut créer ses propres clés.',
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
    'La CLI, les scripts et les agents peuvent se connecter avec une clé d’API au lieu de votre mot de passe. Donnez à chaque clé un nom, une courte durée de validité et uniquement l’accès dont elle a besoin.',
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
    'Les clés à accès personnalisé ne peuvent pas utiliser les macros, installer des extensions, ni créer ou contrôler des serveurs atom, même si elles appartiennent au propriétaire. Une clé à accès complet le peut, si vous le pouvez.',
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
} satisfies CatalogOf<typeof en>;
