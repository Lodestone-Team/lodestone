export default {
  // InstanceCreateForm / ModpackCreateForm: the steps in the dialog's sidebar
  'create.step.selectGame': 'Select Game',
  'create.step.basicSettings': 'Basic Settings',
  'create.step.serverSettings': 'Server Settings',
  'create.step.autoSettings': 'Auto Settings',
  'create.step.selectModpack': 'Select Modpack',
  'create.step.selectVersion': 'Select Version',

  // InstanceCreateForm: the pages' subtitles, buttons and errors
  'create.basicSettings.description': 'Basic settings for your server.',
  'create.serverSettings.description': 'Configure your server.',
  'create.autoSettings.description': 'Automatically configure your server.',
  'create.checkFields': 'Please ensure your fields are filled out correctly',
  /** the button on the last step that creates the server */
  'create.createServer': 'Create Server',
  /** {error}: the core's error message */
  'create.createFailed': 'Error creating server: {error}',
  'create.unknownValueType': 'Error generating form: Unknown value type.',

  // form.ts and ModpackCreateForm: the dashboard's own setup fields
  'create.field.name': 'Name',
  'create.field.nameDescription': 'The name of the server',
  /** the name a new server gets unless the user changes it */
  'create.field.nameDefault': 'My Server',
  'create.field.description': 'Description',
  'create.field.descriptionDescription': 'The description of the server',
  'create.field.port': 'Port',
  'create.field.portDescription': 'The port to run the server on',
  'create.field.minRam': 'Minimum RAM',
  'create.field.minRamDescription':
    'The minimum amount of RAM to allocate to the server, in MB',
  'create.field.maxRam': 'Maximum RAM',
  'create.field.maxRamDescription':
    'The maximum amount of RAM to allocate to the server, in MB',
  'create.field.autoStart': 'Auto Start',
  'create.field.autoStartDescription':
    'The server will start automatically when the application starts',
  'create.field.restartOnCrash': 'Restart on Crash',
  'create.field.restartOnCrashDescription':
    'The server will restart automatically if it crashes',

  // form.ts: validation of a setting; {name} is the setting's name (often sent by the core, untranslated)
  'create.validation.required': '{name} is required',
  'create.validation.number': '{name} must be a number',
  'create.validation.integer': '{name} must be an integer',
  'create.validation.positive': '{name} must be positive',
  'create.validation.min': '{name} must be greater than or equal to {min}',
  'create.validation.max': '{name} must be less than or equal to {max}',
  'create.validation.oneOf': '{name} must be one of the available options',

  // FormComboField: the options list of a field
  /** an option that adds what the user typed; {query} is that text */
  'create.combo.add': 'Add “{query}”',
  'create.combo.nothingFound': 'Nothing found.',

  // GameTypeSelectForm: the first step, picking a game
  'create.gameType.title': 'Select a game',
  'create.gameType.subtitle': 'What will your server be used for?',
  /** the Docker card's title, a link to its wiki page */
  'create.gameType.docker': 'Learn about Docker Server (beta)',
  /** a badge next to the Docker card's title */
  'create.gameType.new': 'NEW!',
  'create.gameType.fetchFailed': 'Error fetching the server',
  'create.gameType.modpack': 'Modrinth modpack',
  'create.gameType.modpackDescription':
    'Create a Fabric server from a Modrinth modpack',

  // GameTypeMappings: games' titles and descriptions
  /** {name}: the game's name, as the core sends it */
  'create.game.genericTitle': '{name} (Generic)',
  'create.game.vanillaDescription':
    'Standard vanilla Minecraft server from Mojang.',
  'create.game.forgeDescription':
    'Modding framework that allows you to install mods and customize your Minecraft experience.',
  'create.game.fabricDescription':
    'Lightweight modding toolchain for Minecraft.',
  'create.game.paperDescription':
    'High-performance Spigot fork that aims to fix gameplay and mechanics inconsistencies.',
  'create.game.spigotDescription':
    'Modified Minecraft server software that supports plugins, offering enhanced performance and customization options.',
  /** {name}: the variant's name, as the core sends it */
  'create.game.otherDescription': 'Unknown Minecraft variant: {name}',
  'create.game.bedrockDescription': 'Minecraft Bedrock Edition server.',
  /** {name}: the game's name, as the core sends it */
  'create.game.genericDescription': 'Unknown game: {name}',

  // CreateInstanceFlow: read by screen readers while the dialog loads
  'create.loadingForm': 'Loading the create form',

  // ModpackCreateForm: creating a server from a Modrinth modpack
  'create.modpack.sourceTitle': 'Select a modpack',
  'create.modpack.sourceDescription':
    'Search Modrinth, paste a Modrinth link, or upload a .mrpack file. Only Fabric modpacks are supported.',
  'create.modpack.searchPlaceholder':
    'Search modpacks, or paste a Modrinth link',
  'create.modpack.onlyFabric': 'Only show Fabric modpacks',
  'create.modpack.upload': 'Upload .mrpack',
  'create.modpack.versionTitle': 'Select a version',
  'create.modpack.versionUploaded': 'The uploaded modpack.',
  'create.modpack.versionDescription': 'Pick the modpack version to install.',
  'create.modpack.downloading': 'Downloading the modpack file to read it…',
  /** {size}: the file's size, e.g. "12.3 MB" */
  'create.modpack.downloadingSize':
    'Downloading the modpack file ({size}) to read it…',
  /** "Create Server" is the label of the last step's button (create.createServer) */
  'create.modpack.versionHint':
    'Select a version to see what it contains. Lodestone downloads its modpack file to read it; nothing is installed until you click Create Server.',
  'create.modpack.settingsTitle': 'Server settings',
  'create.modpack.loadingModpack': 'Loading modpack',
  'create.modpack.loadingVersions': 'Loading versions',
  /** {error}: the core's error message */
  'create.modpack.versionsFailed': 'Failed to load versions: {error}',
  'create.modpack.noVersions': 'This modpack has no versions.',
  'create.modpack.select': 'Select',
  'create.modpack.selected': 'Selected',
  'create.modpack.showMore': {
    one: 'Show more ({count} left)',
    other: 'Show more ({count} left)',
  },
  // the summary of the picked version
  'create.modpack.summary.modpack': 'Modpack',
  'create.modpack.summary.loader': 'Loader',
  'create.modpack.summary.serverFiles': 'Server files',
  /** {count}: the number of files; {size}: their total size, e.g. "120 MB" */
  'create.modpack.summary.serverFilesValue': '{count} ({size} to download)',
  /** {reason}: the core's explanation */
  'create.modpack.unsupported': 'This modpack can’t be used: {reason}',
  // toasts and errors
  /** {error}: the core's error message */
  'create.modpack.inspectFailed': 'Failed to read the modpack: {error}',
  /** {type}: the Modrinth project type as Modrinth names it (mod, resourcepack, shader...) */
  'create.modpack.notAModpack': 'That is a {type}, not a modpack',
  'create.modpack.chooseMrpack': 'Choose a .mrpack file',
  'create.modpack.noUploadId': 'The core returned no upload id',
  /** {error}: the core's error message */
  'create.modpack.uploadFailed': 'Failed to upload the modpack: {error}',
  // validation of the settings step
  'create.modpack.nameRequired': 'Name is required',
  'create.modpack.portNumber': 'Port must be a number',
  'create.modpack.portInteger': 'Port must be an integer',
  'create.modpack.portRange': 'Port must be between 1 and 65535',
  'create.modpack.portRequired': 'Port is required',
  'create.modpack.minRamNumber': 'Minimum RAM must be a number',
  'create.modpack.minRamInteger': 'Minimum RAM must be an integer',
  'create.modpack.minRamMin': 'Minimum RAM must be at least 1 MB',
  'create.modpack.minRamRequired': 'Minimum RAM is required',
  'create.modpack.maxRamNumber': 'Maximum RAM must be a number',
  'create.modpack.maxRamInteger': 'Maximum RAM must be an integer',
  'create.modpack.maxRamMin': 'Maximum RAM must be at least the minimum RAM',
  'create.modpack.maxRamRequired': 'Maximum RAM is required',
} as const;
