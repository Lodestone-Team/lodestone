export default {
  // GameConsole: a server's console tab
  /** the page title, inside common.pageTitle */
  'console.pageTitle': 'Server Console',
  'console.searchPlaceholder': 'Search console…',
  /** a checkbox: the search text is a regular expression */
  'console.regex': 'Regex',
  /** the status dot in the console's corner (tooltip) */
  'console.status.noPermission': 'No permission to access console',
  'console.status.loading': 'Loading console…',
  'console.status.buffered': 'History messages. No live updates',
  'console.status.live': 'Console is live',
  'console.status.liveNoBuffer':
    'Console is live but failed to fetch history. Your internet connection may be unstable',
  'console.status.closed': 'Console is closed',
  'console.status.error': 'Connection lost or error',
  /** the status tooltip and input placeholder when the server is stopped */
  'console.serverStopped': 'Server is stopped',
  /** the command input's placeholder when it can't be used */
  'console.input.noPermission': 'No permission',
  'console.input.placeholder': 'Enter command…',
  'console.noPermission': 'You don’t have permission to access this console',
  /** a screen reader label for the placeholder shown while the console loads */
  'console.loading': 'Loading console',
  'console.empty': 'No console messages yet',
  /** above the console lines; {max} is the number of lines shown */
  'console.hiddenLines': {
    one: '{count} older line hidden (showing the newest {max})',
    other: '{count} older lines hidden (showing the newest {max})',
  },
  /** {error} is the core's error message */
  'console.sendFailed': 'Failed to send command: {error}',

  // ConsoleLine: one line of console output
  /** a tooltip on the Lodestone user's name after a line about a player */
  'console.line.linkedUser':
    'The Lodestone user who linked this Minecraft account',

  // CodeExecution: owner-only features and the code execution switch
  /** "atom" servers run TypeScript code; keep "atom" untranslated */
  'console.ownerOnlyAtom': 'Only the owner can control atom servers',
  /** the same, as a sentence (a setting's read-only hint) */
  'console.ownerOnlyAtomSentence': 'Only the owner can control atom servers.',
  /** why the Java command / arguments settings can't be changed */
  'console.commandLine.readOnly':
    'Read-only while code execution is disabled on this core.',
  'console.commandLine.ownerOnly': 'Only the owner can change this setting.',

  // Macros page
  /** the page title, inside common.pageTitle */
  'console.macros.pageTitle': 'Server Macros',
  'console.macros.intro': 'All macros for your server',
  'console.macros.ownerOnly':
    'Macros are only available to the owner of this core.',
  'console.macros.codeExecutionDisabledBanner':
    'Code execution is disabled on this core. Macros can’t be run or configured until it is turned back on in Core Settings.',
  'console.macros.codeExecutionDisabled':
    'Code execution is disabled on this core',
  /** tabs */
  'console.macros.tab.macros': 'All Macros',
  'console.macros.tab.tasks': 'Running Tasks',
  'console.macros.tab.history': 'History',
  /** table column headers, written in capitals */
  'console.macros.column.name': 'MACRO NAME',
  'console.macros.column.lastRun': 'LAST RUN',
  'console.macros.column.created': 'CREATED',
  'console.macros.column.finished': 'FINISHED',
  'console.macros.column.pid': 'PROCESS ID',
  /** a time in the table that isn't known (e.g. a macro that never ran) */
  'console.macros.noTime': 'N/A',
  /** row menu items */
  'console.macros.run': 'Run Macro',
  'console.macros.editConfig': 'Edit Config',
  'console.macros.killTask': 'Kill Task',
  /** the macro settings dialog */
  'console.macros.configTitle': 'Macro Config',
  'console.macros.configRequired': 'This macro requires additional config.',
  /** toasts; {error} is the core's error message or console.macros.noServer */
  'console.macros.noServer': 'No server selected',
  'console.macros.loadFailed': 'Failed to load macros: {error}',
  'console.macros.openConfigFailed': 'Error opening macro config: {error}',
  'console.macros.runFailed': 'Error running macro: {error}',
  'console.macros.killFailed': 'Failed to kill task: {error}',
  'console.macros.saveConfigFailed': 'Failed to save macro config: {error}',
  'console.macros.invalidState': 'Error: Invalid state',

  // MacroCreateForm (not used yet)
  'console.macros.create.name': 'Macro Name',
  'console.macros.create.args': 'Arguments',
  'console.macros.create.submit': 'Create Macro',
  'console.macros.create.nameRequired': 'A macro name is required',

  // PlayitggOverview: the Playit integration page
  'console.playit.loading': 'Loading playit.gg',
  /** {error} is the core's error message */
  'console.playit.loadFailed': 'Failed to load playit.gg status: {error}',
  'console.playit.signupFailed': 'Failed to generate a signup link: {error}',
  'console.playit.verifyFailed':
    'Failed to verify the playit.gg account: {error}',
  'console.playit.startFailed': 'Failed to start the playit.gg runner: {error}',
  'console.playit.stopFailed': 'Failed to stop the playit.gg runner: {error}',
  /** the sign-up dialog */
  'console.playit.setupTitle': 'Set Up Playitgg',
  'console.playit.setupIntro':
    'You need to sign up for Playitgg before you can use this feature. Generate a signup link with the button below!',
  'console.playit.generateLink': 'Generate Link',
  'console.playit.setupFollowLink':
    'Follow the link below and make an account! After you make the account and get to the dashboard, press Verify below.',
  /** the button named in console.playit.setupFollowLink */
  'console.playit.verify': 'Verify',
  /** the program that runs the tunnels on this computer */
  'console.playit.runnerTitle': 'Playitgg Runner',
  /** the runner's state, a label next to the title */
  'console.playit.runner.Stopped': 'Stopped',
  'console.playit.runner.Loading': 'Loading',
  'console.playit.runner.Started': 'Started',
  /** {running} is console.playit.troubleRunning, in bold */
  'console.playit.trouble':
    'If you’re having trouble, make sure your tunnel is set up correctly on the website and that {running}!',
  'console.playit.troubleRunning': 'your server is actually running',
  'console.playit.signupRequired':
    'You need to sign up for Playitgg before you can use this.',
  'console.playit.signupLink': 'Press here to sign up.',
  'console.playit.tunnels': 'Tunnels',
  'console.playit.refreshTunnels': 'Refresh tunnels',
  /** {link} is a link to the playit.gg website */
  'console.playit.tunnelsIntro':
    'Your tunnels are listed below. You can create more and change the settings on your dashboard on the {link} website.',
  'console.playit.loadingTunnels': 'Loading tunnels',
  /** {address} is an IP address and port, e.g. 127.0.0.1:25565 */
  'console.playit.localServer': 'Local server at {address}',
  'console.playit.copyAddress': 'Copy the address',
  'console.playit.noTunnels': 'You haven’t created any tunnels yet',
  'console.playit.notSignedUp': 'You haven’t signed up yet',
  /** a link to the integration's documentation */
  'console.playit.help': 'Need help?',
} as const;
