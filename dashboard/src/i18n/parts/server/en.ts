// A game server's pages: tabs, overview, performance, players, settings
export default {
  // tabList: the server's tabs in the sidebar, and their headings
  'server.tab.overview': 'Overview',
  'server.tab.settings': 'Settings',
  'server.tab.console': 'Console',
  'server.tab.files': 'Files',
  'server.tab.mods': 'Mods',
  'server.tab.macros': 'Macros',
  'server.tab.logs': 'Event Logs',
  /** the playit.gg tunnel tab: a product name */
  'server.tab.playitgg': 'Playitgg',

  // InstanceTabs
  'server.notFound': 'Server not found',
  /** {path} is the part of the URL naming a tab that doesn't exist */
  'server.tabNotFound': '{path} not found',
  'server.logs.comingSoon': 'Coming soon to a dashboard near you!',

  // InstanceOverview
  'server.overview.pageTitle': 'Server Overview',
  'server.overview.noName': 'No name',
  'server.overview.noDescription': 'No description',
  /** a label under the server name, e.g. "Version 1.20.4" */
  'server.overview.version': 'Version {version}',
  /** a label under the server name: online players / maximum, e.g. "Player Count 3/20" */
  'server.overview.playerCount': 'Player Count {players}/{max}',

  // InstanceDetails: the collapsible list of the server's details
  'server.details.title': 'Details',
  'server.details.name': 'Name',
  'server.details.description': 'Description',
  'server.details.uuid': 'UUID',
  'server.details.game': 'Game',
  'server.details.minecraftVersion': 'Minecraft version',
  'server.details.version': 'Version',
  /** the mod loader, e.g. "Fabric 0.15.3" */
  'server.details.loader': 'Loader',
  'server.details.port': 'Port',
  'server.details.state': 'State',
  'server.details.players': 'Players',
  'server.details.created': 'Created',
  'server.details.autoStart': 'Auto start',
  'server.details.restartOnCrash': 'Restart on crash',
  'server.details.ram': 'Min / Max RAM',
  /** the minimum and maximum memory in MB; "?" when unknown */
  'server.details.ramValue': '{min} MB / {max} MB',
  'server.details.folder': 'Folder',
  'server.details.modpack': 'Created from modpack',

  // InstancePerformance: CPU and memory graphs
  'server.performance.title': 'Server Monitors',
  'server.performance.lead': 'Instance’s usage of your Lodestone Core',
  /** above the CPU graph, in capitals; {value} is a percentage like 12.5 */
  'server.performance.cpuLoad': 'CPU LOAD: {value}%',
  /** above the memory graph, in capitals; {value} is in GiB, like 1.5 */
  'server.performance.memoryUsed': 'MEMORY USED: {value} GiB',
  'server.performance.loadingGraph': 'Loading the graph',
  'server.performance.cpuUsage': 'CPU Usage',
  'server.performance.memoryUsage': 'Memory Usage',
  /** the unit after a graph value, e.g. "50%" (French puts a space before it) */
  'server.performance.unitPercent': '%',
  /** the unit after a graph value, e.g. "1.5GiB" (French puts a space before it) */
  'server.performance.unitGiB': 'GiB',

  // PerformanceGraph
  /** a time on the graph's axis, seconds ago, e.g. "60s" */
  'server.graph.seconds': '{seconds}s',
  /** the time in the graph's tooltip, before the value */
  'server.graph.tooltipTime': '{time}:',

  // InstancePlayerList
  'server.players.title': 'Player List',
  'server.players.lead': 'All players that are currently online',
  /** the sort button above the list, in capitals */
  'server.players.sortByName': 'NAME',
  'server.players.avatarAlt': 'Avatar of {name}',
  'server.players.linkedUser':
    'The Lodestone user who linked this Minecraft account',
  'server.players.none': 'No players online',

  // InstanceSettings
  'server.settings.pageTitle': 'Server Settings',
  'server.settings.automation': 'Automation',
  'server.settings.automationLead':
    'When Lodestone starts this server by itself.',
  'server.settings.autoStart': 'Auto start',
  'server.settings.autoStartHint':
    'Start this server when Lodestone Core starts',
  'server.settings.restartOnCrash': 'Restart on crash',
  'server.settings.restartOnCrashHint': 'Start this server again if it crashes',
  /** a section of the core's settings that has none yet */
  'server.settings.notAvailable': 'Not available at this moment',
  'server.settings.notAvailableHint': 'Try to start this server at least once',
  'server.settings.noPermission': 'No permission',
  'server.settings.dangerZone': 'Danger zone',
  'server.settings.dangerZoneLead':
    'These settings can cause irreversible damage to your server.',
  'server.settings.deleteServer': 'Delete server',
  'server.settings.deleteServerHint':
    'Permanently deletes this server and its data',
  /** the title of the confirmation dialog; {name} is the server's name */
  'server.settings.deleteTitle': 'Delete “{name}”',
  'server.settings.deleteConfirm': 'I understand, delete this server',
  'server.settings.deleteCannotUndo': 'This action cannot be undone.',
  /** {cannotUndo} is server.settings.deleteCannotUndo, in bold */
  'server.settings.deleteWarning':
    '{cannotUndo} This instance’s settings, worlds and backups will be permanently deleted. Please backup any important data before proceeding.',

  // SettingField: one of the settings the core lists
  'server.settingField.unknownType': 'Submission Error: Unknown value type.',

  // PublicListingSetting: what a server shows without signing in
  'server.publicListing.title': 'Sharing',
  'server.publicListing.lead': 'Who can find this server without signing in.',
  'server.publicListing.listedPublicly': 'Listed publicly',
  'server.publicListing.public': 'Public',
  'server.publicListing.ownerOnly': 'Only the owner can change this',
  'server.publicListing.flag.status': 'Status and players',
  'server.publicListing.flag.address': 'Address to join',
  'server.publicListing.flag.clientPack': 'Client mods download',
  /** {list} is a list of the server.publicListing.shown.* items, e.g. "name and version and address" */
  'server.publicListing.shownText':
    'Anyone who can reach this core sees its {list}, without signing in.',
  'server.publicListing.shown.nameVersion': 'name and version',
  'server.publicListing.shown.status': 'status and player count',
  'server.publicListing.shown.address': 'address',
  'server.publicListing.shown.clientPack': 'client mods',
  /** "List servers publicly" is a setting in Core Settings */
  'server.publicListing.turnOnFirst': 'Turn on “List servers publicly” first.',
  /** {shown} is server.publicListing.shownText */
  'server.publicListing.hiddenWhileOff':
    '{shown} Hidden while “List servers publicly” is off.',
  'server.publicListing.notShown': 'Not shown without signing in.',
  'server.publicListing.noServers': 'No Minecraft Java Edition servers yet.',

  // ResourcePackSetting: the server resource pack (server.properties)
  'server.resourcePack.title': 'Resource pack',
  'server.resourcePack.lead':
    'A pack players are asked to download when they join.',
  'server.resourcePack.url': 'URL',
  'server.resourcePack.urlHint':
    'A direct link to the pack zip. Leave it empty for none.',
  'server.resourcePack.sha1Hint':
    'Lets players check the download and cache the pack.',
  'server.resourcePack.sha1Error': 'The SHA-1 is 40 hexadecimal characters.',
  /** computes the SHA-1 of the pack */
  'server.resourcePack.compute': 'Compute',
  /** {size} is the size of the pack, e.g. "1.5 MB" */
  'server.resourcePack.downloaded': 'Downloaded {size}.',
  'server.resourcePack.required': 'Require the pack',
  'server.resourcePack.requiredHint':
    'Players who decline it are disconnected.',
  'server.resourcePack.prompt': 'Prompt',
  'server.resourcePack.promptHint':
    'The message shown when players are asked. Plain text, or a JSON text component.',
  'server.resourcePack.remove': 'Remove pack',
  'server.resourcePack.saved': 'Resource pack saved',
  'server.resourcePack.savedRestart':
    'Resource pack saved. Restart the server to use it.',

  // InstanceCard: the selected server in the sidebar, with its power menu
  'server.action.start': 'Start',
  'server.action.stop': 'Stop',
  'server.action.restart': 'Restart',
  'server.action.kill': 'Kill',
  /** the tooltip of the power button */
  'server.card.power': 'Power',
  'server.card.noScripting':
    'This core was built without scripting support (Deno), so atom servers aren’t available.',
  'server.card.codeExecutionDisabled':
    'Code execution is disabled on this core',
  /** copies the server's address */
  'server.card.copyIp': 'Copy IP',
  /** online players / maximum, e.g. "3/20 Online" */
  'server.card.online': '{players}/{max} Online',

  // InstanceLoadingPill: a server being created, in the sidebar
  /** {percent} is formatted, e.g. "45%" */
  'server.loading.settingUp': 'Setting up… ({percent})',

  // apis.ts: toasts and errors of file and sign-in requests
  'server.api.deletedFile': 'Deleted file: {name}',
  'server.api.deletedDirectory': 'Deleted directory: {path}',
  /** {error} is the error message */
  'server.api.downloadFailed': 'Failed to download {name}: {error}',
  'server.api.uploading': {
    one: 'Uploading {count} file',
    other: 'Uploading {count} files',
  },
  'server.api.moveFailed': 'Failed to move {name}: {error}',
  'server.api.zipFailed': 'Failed to initiate zip: {error}',
  'server.api.zipping': {
    one: 'Zipping {count} item…',
    other: 'Zipping {count} items…',
  },
  'server.api.unzipFailed': 'Failed to unzip {name}: {error}',
  'server.api.unzipping': 'Unzipping {name}…',
  'server.api.tooManyAttemptsSeconds': {
    one: 'Too many failed sign-in attempts. Wait {count} second, then try again.',
    other:
      'Too many failed sign-in attempts. Wait {count} seconds, then try again.',
  },
  'server.api.tooManyAttemptsMinutes': {
    one: 'Too many failed sign-in attempts. Wait {count} minute, then try again.',
    other:
      'Too many failed sign-in attempts. Wait {count} minutes, then try again.',
  },
  'server.api.tooManyAttempts':
    'Too many failed sign-in attempts. Wait a few minutes, then try again.',
  /** the core couldn't be reached; {message} is the reason */
  'server.api.loginFailed': 'Login failed: {message}',
  /** the core refused the sign-in; {message} is its reason */
  'server.api.loginError': 'Error: {message}',
} as const;
