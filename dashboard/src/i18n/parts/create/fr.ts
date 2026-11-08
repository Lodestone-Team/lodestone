import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
  'create.gameType.docker': 'En savoir plus sur les serveurs Docker (bêta)',
  'create.gameType.new': 'NOUVEAU !',
  'create.gameType.fetchFailed': 'Erreur lors de la récupération du serveur',
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
} satisfies CatalogOf<typeof en>;
