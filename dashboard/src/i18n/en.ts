// The English catalog: the source of every message key (see types.ts).
//
// Keys are flat and dotted: <area>.<component>.<name>. A message is a string,
// or plural forms picked by `count` ({ one, other }). `{name}` is a
// placeholder, filled from the params of t(); every catalog must use the same
// placeholders (checked by catalogs.test.ts). A key missing from fr.ts,
// zh-Hans.ts or zh-Hant.ts fails tsc.
//
// Wording: "server" is a game server Lodestone Core runs; "core" is Lodestone
// Core itself. Never call the core a server. Product names (Lodestone,
// Lodestone Core, Minecraft, Modrinth) are never translated. Content
// from the core or a server (names, logs, error messages) isn't in here.
//
// Glossary (the terms each translation uses):
//   server (a game server)
//   core (Lodestone Core)
//   instance
//   dashboard
//   console
//   settings
//   user / owner
//   permission
//   sign in / sign out
//   password / username
//   API key / token
//   file / folder
//   upload / download
//   mod / modpack
//   resource pack
//   player / world
//   port / address
//   memory (RAM)
//   start / stop / restart / kill
//   running / stopped
//   log / event
//   save / delete / remove
//   enable / disable
//   connection / network
//   default / search
//   public listing

export const en = {
  // ===== Common words =====

  'common.cancel': 'Cancel',
  'common.confirm': 'Confirm',
  'common.save': 'Save',
  'common.saved': 'Saved',
  'common.saving': 'Saving…',
  'common.delete': 'Delete',
  'common.remove': 'Remove',
  'common.rename': 'Rename',
  'common.edit': 'Edit',
  'common.create': 'Create',
  'common.add': 'Add',
  'common.close': 'Close',
  'common.back': 'Back',
  'common.next': 'Next',
  'common.done': 'Done',
  'common.copy': 'Copy',
  'common.copied': 'Copied',
  'common.copiedToClipboard': 'Copied to clipboard',
  'common.retry': 'Retry',
  'common.refresh': 'Refresh',
  'common.search': 'Search',
  'common.loading': 'Loading…',
  'common.yes': 'Yes',
  'common.no': 'No',
  'common.ok': 'OK',
  'common.on': 'On',
  'common.off': 'Off',
  'common.enabled': 'Enabled',
  'common.disabled': 'Disabled',
  'common.none': 'None',
  'common.unknown': 'Unknown',
  'common.default': 'Default',
  'common.optional': 'Optional',
  'common.required': 'Required',
  'common.learnMore': 'Learn more',
  'common.showMore': 'Show more',
  'common.showLess': 'Show less',
  'common.you': 'You',
  'common.unknownError': 'Unknown error',
  'common.networkError': 'Network error',
  'common.networkErrorWithMessage': 'Network error: {message}',
  'common.invalidToken': 'Invalid token: try to log out and log in again',
  'common.notAnInteger': 'Not an integer',
  'common.notANumber': 'Not a valid number',
  'common.justNow': 'just now',
  /** a page title in the browser tab, e.g. "Settings - Lodestone" */
  'common.pageTitle': '{page} - Lodestone',
  /** a server's state (InstanceState), e.g. common.state.Running */
  'common.state.Starting': 'Starting',
  'common.state.Running': 'Running',
  'common.state.Stopping': 'Stopping',
  'common.state.Stopped': 'Stopped',
  'common.state.Error': 'Error',
  // Form fields and settings rows (Atoms/Form, Atoms/Config)
  /** a form field’s label above the field, e.g. "Name:" */
  'common.fieldLabel': '{label}:',
  /** a dropdown with no value chosen yet */
  'common.select': 'Select…',
  /** shown under a setting the user isn’t allowed to see */
  'common.noPermission': 'No permission',
  'common.input.cannotBeEmpty': 'Cannot be empty',
  'common.input.mustBeNumber': 'Must be a number',
  /** {min} is the smallest allowed value */
  'common.input.mustBeGreaterThan': 'Must be greater than {min}',
  /** {max} is the largest allowed value */
  'common.input.mustBeLessThan': 'Must be less than {max}',
  /** a combo box option that adds what the user typed, {query} */
  'common.combo.addCustom': 'Add “{query}”',
  /** a combo box search that matches no option */
  'common.combo.nothingFound': 'Nothing found.',
  // Table
  /** an empty table */
  'common.table.noData': 'No data currently available',
  // ClipboardTextfield: text that copies itself when clicked
  /** tooltip */
  'common.clipboard.copyToClipboard': 'Copy to clipboard',
  /** a toast; {text} is what was copied, e.g. an address */
  'common.clipboard.copiedText': 'Copied “{text}”',
  // The server resource pack URL check (utils/resourcePack.ts)
  'common.resourcePack.urlSpaces': 'The URL cannot contain spaces.',
  'common.resourcePack.notAUrl': 'That is not a URL.',
  'common.resourcePack.urlScheme':
    'The URL must start with http:// or https://.',

  // ===== App shell: navigation, top bar, settings frame, sign-in and setup, notifications =====

  // App (pages/index.tsx): the session and its token
  'shell.session.expired': 'Your session has expired, please sign in again',
  'shell.session.invalid':
    'Your session is no longer valid, please sign in again',
  'shell.token.invalid': 'Invalid token',
  /** "exp" is the token's expiry claim, a technical name */
  'shell.token.invalidExp': 'Invalid exp in token',
  /** "uid" is the token's user id claim, a technical name */
  'shell.token.invalidUid': 'Invalid uid in token',
  'shell.token.expired': 'Token expired',

  // Errors the dashboard writes when a request answers unexpectedly
  'shell.error.invalidStatusCode': 'Invalid status code',
  'shell.error.invalidResponse': 'Invalid response',

  // Settings modal: the pages
  'shell.settings.general': 'General',
  'shell.settings.users': 'Users',
  'shell.settings.profile': 'Profile',

  // SettingsLayout: the settings modal
  /** the button that opens the list of settings pages in a narrow window */
  'shell.settings.pagesButton': 'Settings pages',
  'shell.settings.close': 'Close settings',
  /** tooltip of the close button; Esc is the keyboard key */
  'shell.settings.closeTooltip': 'Close (Esc)',

  // SettingsLeftNav: the settings sidebar
  'shell.settingsNav.label': 'Settings',
  /** caption over the core's settings pages */
  'shell.settingsNav.core': 'Core',
  /** caption over the user's own settings pages */
  'shell.settingsNav.account': 'Account',
  /** caption over the list of users */
  'shell.settingsNav.users': 'Users',
  'shell.settingsNav.unselectUser': 'Unselect user',

  // DashboardLayout: dialogs about the core
  'shell.beta.title': 'Thanks for using Lodestone beta!',
  'shell.beta.body':
    'Thanks for using Lodestone beta! If you find any issues please let us know on our Discord server or on Github.',
  'shell.versionMismatch.title': 'Update Required!',
  'shell.versionMismatch.continue': 'I understand, continue without updating',
  'shell.versionMismatch.coreVersion': 'Core Version:',
  'shell.versionMismatch.dashboardVersion': 'Dashboard Version:',
  'shell.versionMismatch.incompatible':
    'Your dashboard and core is incompatible!',
  /** {link} is a link whose text is shell.versionMismatch.wiki */
  'shell.versionMismatch.body':
    'This can cause unexpected behavior. Please update your core to the latest version. Visit {link} for more information.',
  'shell.versionMismatch.wiki': 'the wiki',
  /** {core} and {dashboard} are version numbers */
  'shell.versionMismatch.toast':
    'Version mismatch! Is your core out of date? Core: {core}, Dashboard: {dashboard}',
  'shell.localCore.title': 'New Local Core Detected',
  /** {name} is the core's name */
  'shell.localCore.body':
    'Detected a local core that is not setup yet. Would you like to setup {name}?',
  /** button: set up the core */
  'shell.localCore.setup': 'Setup',
  'shell.localCore.skip': 'Skip',
  'shell.setupRequired.title': 'Setup Required',
  /** {name} is the core's name */
  'shell.setupRequired.body':
    '{name} is not setup yet. Please complete the setup process.',
  'shell.setupRequired.changeCore': 'Change Core',
  'shell.coreError.title': 'Core Connection Error',
  /** {address} is the core's address and port, e.g. 192.168.0.2:16662 */
  'shell.coreError.body':
    'There was an error connecting to {address}. Please select a different core, refresh the page, or simply wait for the core to come back online.',
  'shell.coreError.back': 'Back to Core selection',
  'shell.coreError.continue': 'Continue with current Core',

  // TopNav: the top bar
  /** {refresh} is a link whose text is shell.topNav.refresh */
  'shell.topNav.degraded': '{refresh} to get the latest data on Firefox.',
  'shell.topNav.refresh': 'Refresh',
  /** tooltip of the DEV badge next to the logo */
  'shell.topNav.devBuild': 'Local development build',

  // CoreConnectionStatus: the core's name and connection status in the sidebar
  'shell.connection.connected': 'Connected',
  'shell.connection.connectedDetail': 'Receiving live updates',
  'shell.connection.connecting': 'Connecting…',
  'shell.connection.connectingDetail': 'Waiting for the core to respond',
  'shell.connection.degraded': 'Degraded',
  'shell.connection.degradedDetail':
    'Live updates unavailable, refresh to update',
  'shell.connection.disconnected': 'Disconnected',
  'shell.connection.disconnectedDetail': 'Cannot reach the core',
  'shell.connection.address': 'Address',
  'shell.connection.version': 'Version',
  /** the core's machine (host name) */
  'shell.connection.host': 'Host',
  /** the core's operating system and architecture */
  'shell.connection.system': 'System',
  'shell.connection.uptime': 'Uptime',
  /** short uptime, e.g. "3d 4h" */
  'shell.connection.uptimeDays': '{days}d {hours}h',
  /** short uptime, e.g. "4h 12m" */
  'shell.connection.uptimeHours': '{hours}h {minutes}m',
  /** short uptime, e.g. "12m" */
  'shell.connection.uptimeMinutes': '{minutes}m',
  /** the core's name, when it isn't known yet */
  'shell.connection.coreFallback': 'Core',
  /** screen reader label: {core} is the core's name, {status} e.g. "Connected" */
  'shell.connection.ariaLabel': '{core}: {status}',

  // LeftNav, InstanceList, IntegrationsList, SelectedInstanceInfo: the sidebar
  /** caption over the server list (shown in capitals) */
  'shell.nav.allInstances': 'All instances',
  /** caption (shown in capitals) */
  'shell.nav.integrations': 'Integrations',
  /** caption over the selected server and its tabs (shown in capitals) */
  'shell.nav.selectedInstance': 'Selected instance',
  'shell.nav.noSelection': 'Any selected server will appear here',
  'shell.nav.loadingServers': 'Loading servers',
  'shell.nav.logInToView': 'Log in to view your servers.',
  'shell.nav.newServer': 'New server…',
  /** the "New server" button, for a user who may not create servers */
  'shell.nav.noPermission': 'No permission',

  // NotificationPanel
  'shell.notifications.title': 'Notifications',
  'shell.notifications.inProgress': 'In progress',
  'shell.notifications.all': 'All Notifications',
  'shell.notifications.loading': 'Loading notifications',
  'shell.notifications.empty': 'No notifications at the moment!',

  // EventStream: notifications built from the core's events
  /** {name} is a server's name */
  'shell.event.state.Starting': 'Server {name} is starting!',
  'shell.event.state.Running': 'Server {name} started!',
  'shell.event.state.Stopping': 'Server {name} is stopping!',
  'shell.event.state.Stopped': 'Server {name} stopped!',
  'shell.event.state.Error': 'Server {name} encountered an error!',
  'shell.event.warningToast':
    'Warning: A server has encountered a warning. This shouldn’t happen, please report this to the developers.',
  'shell.event.warning': 'Server {name} encountered a warning',
  'shell.event.error': 'Server {name} encountered an error',
  /** {players} is a list of player names, {name} a server's name */
  'shell.event.playersJoined': '{players} Joined {name}',
  'shell.event.playersLeft': '{players} Left {name}',
  'shell.event.playersJoinedAndLeft':
    '{joined} Joined {name} and {left} Left {name}',
  /** {player} said {message} (a chat message) on the server {name} */
  'shell.event.modpackUpdate': '{pack} {version} is out for {name}',
  'shell.event.playerMessage': '{player} said {message} on {name}',
  /** {name} is the name of an API key */
  'shell.event.apiKeyCreated': 'API key “{name}” created',
  'shell.event.apiKeyRevoked': 'API key “{name}” revoked',
  /** {error} is an error message */
  'shell.event.loadFailed': 'Failed to load notifications: {error}',

  // UserMenu: the account menu at the bottom of the sidebar
  /** shown instead of a user name when nobody is signed in */
  'shell.userMenu.guest': 'Guest',
  /** screen reader label of the account button, {name} is a user name */
  'shell.userMenu.account': 'Account: {name}',
  /** {name} is the user name */
  'shell.userMenu.signedInAs': 'Signed in as {name}',
  'shell.userMenu.notSignedIn': 'Not signed in',
  'shell.userMenu.signOut': 'Sign out',
  'shell.userMenu.signIn': 'Sign in',
  'shell.userMenu.changeCore': 'Change core',
  'shell.userMenu.settings': 'Settings',

  // UserBox: a user in a list, with its menu
  'shell.userBox.changePassword': 'Change password',
  'shell.userBox.cantDeleteSelfTitle': 'You can’t delete yourself',
  'shell.userBox.cantDeleteSelf': 'You can’t delete yourself.',
  'shell.userBox.deleteTitle': 'Delete User',
  /** {name} is a user name */
  'shell.userBox.deleteConfirm':
    'Are you sure you want to delete {name}? This action cannot be undone.',
  'shell.userBox.deleteUser': 'Delete user',
  /** a tag next to a user name */
  'shell.userBox.admin': 'Admin',
  /** a tag next to a user name */
  'shell.userBox.owner': 'Owner',

  // CoreConnectionHelp: help after a failed connection to a remote core
  /** {link} is the address of the core's certificate page */
  'shell.connectionHelp.trustCertificate':
    'If the core is running and uses its built-in certificate, this browser has to trust that certificate once. Open {link}, accept the certificate warning, then come back and try again.',
  'shell.connectionHelp.fingerprint':
    'To make sure it is your core, compare the certificate fingerprint your browser shows with the one in the core’s log or in its Core Settings.',
  'shell.connectionHelp.plainHttp':
    'Lodestone cores refuse plain HTTP from other devices by default.',
  'shell.connectionHelp.savedCore':
    'Connect a new core with the same address and the https protocol.',
  'shell.connectionHelp.changeProtocol':
    'Change the protocol to https and try again.',

  // Login and setup pages (src/pages/login)
  /** {error} is an error message, shown in bold */
  'shell.login.checkFields':
    '{error}: Please ensure your fields are filled out correctly.',
  'shell.login.invalidResponse': 'Invalid response, setup may be invalid',
  'shell.login.downloadCore': 'Download Lodestone Core',
  'shell.login.continue': 'Continue',
  /** between two choices: pick a core OR connect a new one */
  'shell.login.or': 'OR',
  'shell.login.username': 'Username',
  'shell.login.password': 'Password',
  'shell.login.usernameRequired': 'Username is required',
  'shell.login.passwordRequired': 'Password is required',
  'shell.login.changeCore': 'Change Core',
  'shell.login.signIn': 'Sign in',
  'common.localOnly':
    'This core only allows apps on its own computer. Open Lodestone Host on that computer to manage it, or set up an owner account there (Settings › General › Allow access from other devices) to use the dashboard.',
  'shell.localOnly.title': 'This core only allows its own computer',
  'shell.login.localOnly':
    'This core only allows apps on this computer. Open Lodestone Host to manage it, or set an owner password there (Settings › General › Allow access from other devices) to use the dashboard.',

  // CoreConnect: adding a core
  'shell.login.withApiKey': 'Sign in with an API key',
  'shell.login.withPassword': 'Sign in with a username and password',
  'shell.login.apiKey': 'API key',
  'shell.login.apiKeyHint':
    'Paste a key that starts with lsk_. You get the key’s access, and revoking the key signs you out.',
  'shell.login.apiKeyRequired': 'An API key is required',
  'shell.login.apiKeyFormat': 'An API key starts with lsk_',
  'shell.coreConnect.pageTitle': 'Connect to Core',
  'shell.coreConnect.title': 'Add a new core',
  'shell.coreConnect.httpErrorTitle': 'HTTP Error',
  /** {link} is a "Learn more" link */
  'shell.coreConnect.mixedContent':
    'Missing site permissions. Enable mixed content settings or disable any HTTPS browser plugins, then refresh the page. {link}',
  /** {link} is a "Learn more" link */
  'shell.coreConnect.httpsWarning':
    'You may need to adjust your network and browser settings. {link}',
  'shell.coreConnect.apiVersion': 'API Version',
  'shell.coreConnect.protocol': 'Protocol',
  'shell.coreConnect.port': 'Port',
  'shell.coreConnect.address': 'IP Address/Domain',
  'shell.coreConnect.addressPlaceholder':
    'e.g. 123.123.123.123 or “myserver.com”',
  'shell.coreConnect.submit': 'Add and Continue',

  // CoreSelectExisting: picking a saved core
  'shell.coreSelect.pageTitle': 'Select Core',
  'shell.coreSelect.title': 'Select Lodestone Core',
  'shell.coreSelect.existing': 'Select Existing Core',
  'shell.coreSelect.connectNew': 'Connect a new core',

  // CoreSetupNew: creating the owner's account on a new core
  'shell.coreSetup.pageTitle': 'Setup new core',
  'shell.coreSetup.title': 'Create an owner’s account',
  /** "First time setup key" is quoted from the core's (English) console output */
  'shell.coreSetup.findKey':
    'Check the console output of the core to find the “First time setup key”',
  'shell.coreSetup.setupKey': 'Setup key',
  'shell.coreSetup.setupKeyPrefilled': 'Setup key (prefilled)',
  'shell.coreSetup.confirmPassword': 'Confirm Password',
  'shell.coreSetup.submit': 'Submit',
  'shell.coreSetup.passwordConfirmRequired':
    'Password confirmation is required',
  'shell.coreSetup.passwordMismatch':
    'Password confirmation must match password',
  'shell.coreSetup.setupKeyRequired': 'Setup key is required',
  'shell.coreSetup.failed': 'Something went wrong while setting up the core',

  // CoreConfigNew: naming a new core
  'shell.coreConfig.pageTitle': 'Configure new core',
  'shell.coreConfig.title': 'Customize your core',
  'shell.coreConfig.coreName': 'Core Name',
  'shell.coreConfig.domain': 'Public Domain/IP (Optional)',
  'shell.coreConfig.coreNameRequired': 'Core name is required',
  /** {max} is a number of characters */
  'shell.coreConfig.domainTooLong':
    'Domain cannot be longer than {max} characters',

  // FirstTime: the welcome page
  /** the browser tab's title */
  'shell.welcome.pageTitle': 'Welcome to Lodestone',
  'shell.welcome.wiki': 'Wiki & FAQ',
  'shell.welcome.title': 'Welcome to Lodestone!',
  /** {version} is the dashboard's version, e.g. 0.5.1 */
  'shell.welcome.version': 'VERSION {version}',
  'shell.welcome.beta': 'Lodestone is a beta software!',
  /** {github} is a link to the issues on GitHub, its text is "Github" */
  'shell.welcome.betaBody':
    'Browser support is limited and bugs are expected. Feature stability and future compatibility is not guaranteed. You can check known issues and report any new ones on our {github}.',
  'shell.welcome.connectExisting': 'Connect to existing Core',
  'shell.welcome.setupCore': 'Setup Lodestone Core',

  // UserLogin and UserSelectExisting: signing in
  'shell.userLogin.switchAccount': 'Switch Account',
  'shell.userSelect.pageTitle': 'Select user',
  'shell.userSelect.loadingUser': 'Loading the user',
  /** {name} is a user name */
  'shell.userSelect.continueAs': 'Continue as {name}',
  'shell.userSelect.continueAsCurrent': 'Continue as current user',
  'shell.userSelect.switchUser': 'Switch user',

  // Home page
  'shell.home.pageTitle': 'Home',
  'shell.home.title': 'Manage your servers',
  'shell.home.subtitle':
    'To start, select a server from the menu or create a new one',
  'shell.home.create': 'Create a new server',

  // 404 page
  'shell.notFound.title': 'Page Not Found',
  'shell.notFound.goHome': 'Go Home',

  // ===== Settings: core settings, users, profile, API keys =====

  // Profile → Appearance → Language
  'settings.language.label': 'Language',
  /** {language} is the browser's language, in that language, e.g. "Français" */
  'settings.language.auto': 'Browser default ({language})',

  // Profile → Appearance → Theme (a segmented control)
  'settings.theme.label': 'Theme',
  'settings.theme.system': 'System',
  'settings.theme.light': 'Light',
  'settings.theme.dark': 'Dark',
  'settings.theme.systemHint': 'Follows your device’s light or dark setting',
  'settings.theme.lightHint': 'Always light',
  'settings.theme.darkHint': 'Always dark',

  // SettingsWithToc: the list of the page's sections, on the left
  'settings.toc.label': 'Sections',

  // Legacy settings page (no longer in use)
  'settings.legacy.title': 'Core Settings',

  // Core settings (Lodestone Core itself, not a game server)
  /** the browser tab title, before " - Lodestone" */
  'settings.core.documentTitle': 'Lodestone Core Settings',
  'settings.core.title': 'General',
  'settings.core.section.core': 'Core',
  'settings.core.section.public': 'Public servers',
  'settings.core.section.publicLead': 'Visible without signing in.',
  'settings.core.section.info': 'Information',
  'settings.core.section.danger': 'Danger zone',
  'settings.core.section.dangerLead': 'Change these with care.',
  'settings.core.name.label': 'Core name',
  'settings.core.name.empty': 'Name cannot be empty',
  'settings.core.name.tooLong': 'Name cannot be longer than {max} characters',
  'settings.core.domain.label': 'Public domain or IP',
  'settings.core.domain.tooLong':
    'Domain cannot be longer than {max} characters',
  'settings.core.domain.tooltip':
    'This is your public IP if you are port-forwarded. If not, leave this as is.',
  'settings.core.modpackChecks.label': 'Check for modpack updates',
  'settings.core.modpackChecks.hint':
    'Every few hours, look on Modrinth for newer versions of the modpacks servers were made from, and say when one is out. Check for an update on a server’s Mods tab still works when this is off.',
  'settings.core.listPublicly.label': 'List servers publicly',
  'settings.core.listPublicly.hint':
    'Anyone who can reach this core sees the servers you make public below.',
  'settings.core.localClaim.label':
    'Let apps on this computer claim owner access',
  'settings.core.localClaim.hint':
    'Lodestone Launcher can become the owner by proving it can write to this core’s data folder. Anything that can write there can already change every user and setting, so this only matters on a shared computer.',
  'settings.core.safeMode.label': 'Safe mode',
  'settings.core.safeMode.hint':
    'Keeps admin access to this machine for the owner only.',
  'settings.core.safeMode.offTitle': 'Turn off safe mode?',
  'settings.core.safeMode.offBody':
    'Are you sure you want to turn off safe mode? This will allow you to give users other than yourself the ability to run potentially dangerous commands. Make sure you trust all the users you give these permissions to.',
  /** the confirm button of the "Turn off safe mode / code execution?" dialogs */
  'settings.core.turnOff': 'Turn off',
  'settings.core.codeExecution.label': 'Code execution',
  'settings.core.codeExecution.hint': 'Custom Java commands.',
  'settings.core.codeExecution.locked':
    'Locked by this core’s administrator (LODESTONE_DISABLE_CODE_EXECUTION or --disable-code-execution).',
  'settings.core.codeExecution.offTitle': 'Turn off code execution?',
  'settings.core.codeExecution.offBody':
    'Custom Java commands will not run on this core until you turn code execution back on. Nothing is started again automatically.',
  'settings.core.lanHttp.label': 'Allow LAN access over plain HTTP',
  'settings.core.lanHttp.hint':
    'Not encrypted. Others on your network can read passwords.',
  'settings.core.lanHttp.confirmTitle': 'Allow plain HTTP from other devices?',
  'settings.core.lanHttp.confirmBody':
    'Devices on your network will be able to connect over plain HTTP. Passwords and session tokens sent that way cross the network unencrypted, so anyone else on the network can read them and take over accounts on this core. Only allow this if a device cannot use https://.',
  'settings.core.lanHttp.allow': 'Allow',
  /** a warning at the top of the page; {setting} is settings.core.lanHttp.label */
  'settings.core.lanHttp.warningOwner':
    'Other devices can connect to this core over plain HTTP, so passwords and session tokens can cross your network unencrypted. Turn off "{setting}" below and connect with https:// instead.',
  'settings.core.lanHttp.warningOther':
    'Other devices can connect to this core over plain HTTP, so passwords and session tokens can cross your network unencrypted. Connect with https:// and ask the owner to turn off plain HTTP access.',
  'settings.core.openPort.label': 'Open port',
  /** {upnp} is a link with the text "UPnP" */
  'settings.core.openPort.hint': 'Port forward on your router with {upnp}.',
  'settings.core.openPort.noPermission': 'No permission',
  'settings.core.openPort.button': 'Open port',
  'settings.core.openPort.portLabel': 'Port to open',
  'settings.core.openPort.portNotNumber': 'Port must be a number',
  'settings.core.openPort.portRequired': 'Port is required',
  'settings.core.openPort.portRange': 'Port must be between 1 and 65535',
  'settings.core.openPort.opened': 'Port {port} opened',
  'settings.core.info.coreVersion': 'Core version',
  'settings.core.info.dashboardVersion': 'Dashboard version',
  'settings.core.info.os': 'OS',
  'settings.core.info.arch': 'Architecture',
  'settings.core.info.cpu': 'CPU',
  'settings.core.info.cpuCores': 'CPU cores',
  'settings.core.info.totalRam': 'Total RAM',
  /** shown as the total RAM when the core reports none (a joke) */
  'settings.core.info.noRam': 'No RAM? How are you running Lodestone?',
  'settings.core.info.httpsCertificate': 'HTTPS certificate',
  'settings.core.info.fingerprint': 'Certificate fingerprint (SHA-256)',
  'settings.core.tls.selfSigned': 'Built-in self-signed certificate',
  'settings.core.tls.userProvided': 'Your own certificate (tls/cert.pem)',
  'settings.core.tls.none': 'Unavailable, plain HTTP only',

  // User settings: the users of the core and their permissions
  /** the browser tab title, before " - Lodestone" */
  'settings.users.documentTitle': 'User Settings',
  'settings.users.title': 'Users',
  'settings.users.description': 'Accounts on this core and what each can do.',
  'settings.users.allUsers': 'All users',
  'settings.users.allMembers': 'All members',
  'settings.users.allMembersCount': 'All members ({count})',
  'settings.users.createUser': 'Create new user',
  'settings.users.createTitle': 'Create a new user account',
  'settings.users.createLead': 'Starts with no permissions.',
  'settings.users.loadingUsers': 'Loading users',
  'settings.users.loadingUser': 'Loading the user',
  'settings.users.loadingServers': 'Loading servers',
  'settings.users.permissions': 'Permissions',
  'settings.users.permissionsLead': 'What {username} can do on this core.',
  'settings.users.unsafePermissions': 'Unsafe permissions',
  'settings.users.unsafePermissionsLead':
    'Turn off safe mode to grant these to non-owners.',

  // Permissions: the title, and what it allows a user or an API key
  'settings.permission.can_create_instance.title': 'Create Servers',
  'settings.permission.can_create_instance.user':
    'The user can create new servers.',
  'settings.permission.can_create_instance.key':
    'The key can create new servers.',
  'settings.permission.can_delete_instance.title': 'Delete Servers',
  'settings.permission.can_delete_instance.user':
    'The user can delete any server.',
  'settings.permission.can_delete_instance.key':
    'The key can delete any server.',
  'settings.permission.can_read_global_file.title': 'Read Global Files',
  'settings.permission.can_read_global_file.user':
    'The user can read any file on the computer Lodestone Core is running on.',
  'settings.permission.can_read_global_file.key':
    'The key can read any file on the computer Lodestone Core is running on.',
  'settings.permission.can_view_instance.title': 'View Servers',
  'settings.permission.can_view_instance.user':
    'The user can see these servers.',
  'settings.permission.can_view_instance.key': 'The key can see these servers.',
  'settings.permission.can_start_instance.title': 'Start Servers',
  'settings.permission.can_start_instance.user':
    'The user can start these servers.',
  'settings.permission.can_start_instance.key':
    'The key can start these servers.',
  'settings.permission.can_stop_instance.title': 'Stop Servers',
  'settings.permission.can_stop_instance.user':
    'The user can stop these servers.',
  'settings.permission.can_stop_instance.key':
    'The key can stop these servers.',
  'settings.permission.can_access_instance_console.title':
    'Access Server Console',
  'settings.permission.can_access_instance_console.user':
    'The user can read and send commands to the console of these servers. This essentially gives operator access to the server.',
  'settings.permission.can_access_instance_console.key':
    'The key can read and send commands to the console of these servers. This essentially gives operator access to the server.',
  'settings.permission.can_access_instance_setting.title':
    'Access Server Settings',
  'settings.permission.can_access_instance_setting.user':
    'The user can read and change the settings of these servers.',
  'settings.permission.can_access_instance_setting.key':
    'The key can read and change the settings of these servers.',
  'settings.permission.can_read_instance_resource.title':
    'Read Server Resources',
  'settings.permission.can_read_instance_resource.user':
    'The user can read the resources (worlds, mods, etc.) of these servers.',
  'settings.permission.can_read_instance_resource.key':
    'The key can read the resources (worlds, mods, etc.) of these servers.',
  'settings.permission.can_read_instance_file.title': 'Read Server Files',
  'settings.permission.can_read_instance_file.user':
    'The user can read the files of these servers.',
  'settings.permission.can_read_instance_file.key':
    'The key can read the files of these servers.',
  'settings.permission.can_write_instance_resource.title':
    'Modify Server Resources',
  'settings.permission.can_write_instance_resource.user':
    'The user can modify the resources (worlds, mods, etc.) of these servers.',
  'settings.permission.can_write_instance_resource.key':
    'The key can modify the resources (worlds, mods, etc.) of these servers.',
  'settings.permission.can_write_instance_file.title': 'Modify Server Files',
  'settings.permission.can_write_instance_file.user':
    'The user can modify the files of these servers.',
  'settings.permission.can_write_instance_file.key':
    'The key can modify the files of these servers.',
  'settings.permission.can_write_global_file.title': 'Modify Global Files',
  'settings.permission.can_write_global_file.user':
    'The user can modify any file on the computer Lodestone Core is running on.',
  'settings.permission.can_write_global_file.key':
    'The key can modify any file on the computer Lodestone Core is running on.',
  'settings.permission.can_manage_permission.title': 'Manage Permissions',
  'settings.permission.can_manage_permission.user':
    'The user can manage the permissions of other users.',
  'settings.permission.can_manage_permission.key':
    'The key can manage the permissions of other users.',

  // Profile page
  /** the browser tab title, before " - Lodestone" */
  'settings.profile.documentTitle': 'User Profile',
  'settings.profile.title': 'Profile',
  /** the username shown when not logged in */
  'settings.profile.guest': 'Guest',
  'settings.profile.notLoggedIn':
    'You’re not logged in! Please log in to access this page.',
  'settings.profile.account': 'Account',
  'settings.profile.username': 'Username',
  'settings.profile.password': 'Password',
  /** a button and the title of its dialog */
  'settings.profile.changePassword': 'Change password',
  'settings.profile.appearance': 'Appearance',
  'settings.profile.appearanceLead': 'Saved in this browser, not on the core.',
  'settings.profile.coloredConsole': 'Colored console',

  // Password and new user forms
  'settings.form.username': 'Username',
  'settings.form.password': 'Password',
  'settings.form.oldPassword': 'Old Password',
  'settings.form.confirmPassword': 'Confirm Password',
  'settings.form.usernameRequired': 'Username is required',
  'settings.form.passwordRequired': 'Password is required',
  'settings.form.confirmRequired': 'Password confirmation is required',
  'settings.form.passwordsMustMatch': 'Passwords must match',
  'settings.form.changePassword': 'Change Password',
  'settings.form.createUser': 'Create User',

  // Linked Minecraft accounts (profile page and user settings)
  'settings.minecraft.title': 'Minecraft accounts',
  'settings.minecraft.ownLead': 'Linked from Lodestone Launcher.',
  'settings.minecraft.ownEmpty':
    'No Minecraft account is linked. Sign in to this core in Lodestone Launcher to link one.',
  'settings.minecraft.userLead':
    'The Minecraft accounts {username} linked from Lodestone Launcher.',
  'settings.minecraft.userEmpty':
    '{username} has no linked Minecraft accounts.',
  'settings.minecraft.tooOld':
    'This core is too old to link Minecraft accounts.',
  'settings.minecraft.removeTitle': 'Remove Minecraft account',
  /** {name} is the Minecraft player name */
  'settings.minecraft.removeBody':
    'Remove the link to {name}? It can be linked again from Lodestone Launcher.',
  'settings.minecraft.removed': 'Removed {name}',

  // API keys (profile page and user settings)
  'settings.apiKeys.title': 'API keys',
  'settings.apiKeys.ownLead':
    'Let the CLI, scripts and agents act as you without your password.',
  'settings.apiKeys.ownEmpty': 'You have no API keys.',
  'settings.apiKeys.userLead':
    'Keys {username} created. Revoking one cuts off what uses it.',
  'settings.apiKeys.userEmpty': '{username} has no API keys.',
  /** a button and the title of its dialog */
  'settings.apiKeys.create': 'Create API key',
  'settings.apiKeys.created': 'Created API key "{name}"',
  'settings.apiKeys.loading': 'Loading API keys',
  'settings.apiKeys.revoke': 'Revoke',
  'settings.apiKeys.revokeTitle': 'Revoke API key',
  /** {display} is the start of the key, e.g. "lsk_ab12…" */
  'settings.apiKeys.revokeBody':
    'Revoke "{name}" ({display})? Anything using this key loses access at once. This can’t be undone.',
  'settings.apiKeys.revoked': 'Revoked API key "{name}"',
  /** {date} is a date, e.g. "Nov 8, 2026" */
  'settings.apiKeys.createdOn': 'Created {date}',
  'settings.apiKeys.expiresOn': 'Expires {date}',
  'settings.apiKeys.expiredOn': 'Expired {date}',
  'settings.apiKeys.expiresNever': 'Expires: Never',
  /** {time} is a relative time, e.g. "3 minutes ago" */
  'settings.apiKeys.lastUsed': 'Last used: {time}',
  'settings.apiKeys.lastUsedNever': 'Last used: Never',
  'settings.apiKeys.scope.full': 'Full access',
  /** the access of a key a Lodestone Launcher device uses */
  'settings.apiKeys.scope.launcher': 'Launcher: view, start and stop servers',
  'settings.apiKeys.scope.launcherConsole':
    'Launcher: view, start and stop servers, read and use the console',
  'settings.apiKeys.scope.launcherConsoleRead':
    'Launcher: view, start and stop servers, read the console',
  'settings.apiKeys.scope.launcherConsoleWrite':
    'Launcher: view, start and stop servers, send console commands',
  /** {servers} is settings.apiKeys.scope.servers, {permissions} is settings.apiKeys.scope.permissions */
  'settings.apiKeys.scope.custom': 'Custom: {servers}, {permissions}',
  'settings.apiKeys.scope.servers': {
    one: '{count} server',
    other: '{count} servers',
  },
  'settings.apiKeys.scope.permissions': {
    one: '{count} permission',
    other: '{count} permissions',
  },
  'settings.apiKeys.passwordSessionOnly':
    'API keys can only be managed when you sign in with your password, not with an API key.',
  // Create API key dialog
  'settings.apiKeys.createLead':
    'Give each key a short expiry and only the access it needs.',
  'settings.apiKeys.name': 'Name',
  'settings.apiKeys.nameRequired': 'Name is required',
  'settings.apiKeys.nameTooLong': 'Name must be at most {max} characters',
  'settings.apiKeys.expiresIn': 'Expires in',
  'settings.apiKeys.expiry.30d': '30 days',
  'settings.apiKeys.expiry.90d': '90 days',
  'settings.apiKeys.expiry.1y': '1 year',
  'settings.apiKeys.expiry.never': 'Never',
  'settings.apiKeys.neverWarning':
    'A key that never expires works until you revoke it. Choose an expiry unless you really need it.',
  'settings.apiKeys.access': 'Access',
  'settings.apiKeys.access.full': 'Full access (everything you can do)',
  'settings.apiKeys.access.custom': 'Custom',
  'settings.apiKeys.chooseOne': 'Choose at least one permission.',
  'settings.apiKeys.createKey': 'Create key',
  // Custom access: the permission editor
  'settings.apiKeys.onlyYours':
    'A key can only have permissions you have yourself. The permissions and servers you don’t have are grayed out.',
  'settings.apiKeys.notYours': 'You don’t have this permission.',
  'settings.apiKeys.noServers': 'There are no servers you can see.',
  'settings.apiKeys.unsafe': 'Unsafe Settings',
  'settings.apiKeys.unsafeSafeMode':
    'Safe mode is on, so the core refuses these to keys with custom access until safe mode is turned off in core settings.',
  // The new key dialog, shown once
  'settings.apiKeys.newTitle': 'API key "{name}" created',
  'settings.apiKeys.newCopyNow':
    'Copy this key now. You won’t be able to see it again.',
  'settings.apiKeys.newUseWithCli': 'Use it with the Lodestone CLI:',
  'settings.apiKeys.newStore':
    'Store the key like a password. Anyone who has it can use Lodestone with its access until it expires or you revoke it.',
  'settings.apiKeys.keyCopied': 'API key copied to the clipboard',
  'settings.apiKeys.cliCopied': 'CLI example copied to the clipboard',
  'settings.apiKeys.copyFailed':
    'Couldn’t copy to the clipboard. Select the text and copy it by hand.',

  // ===== Server pages: tabs, overview, settings, performance, players =====

  // tabList: the server's tabs in the sidebar, and their headings
  'server.tab.overview': 'Overview',
  'server.tab.settings': 'Settings',
  'server.tab.console': 'Console',
  'server.tab.files': 'Files',
  'server.tab.mods': 'Mods',
  'server.tab.logs': 'Event Logs',

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
  'server.share.title': 'Share',
  'server.share.lead': 'People with the link join from Lodestone Launcher',
  'server.share.off':
    'Send a link to anyone you want to play with. They open it in Lodestone Launcher, and their game connects straight to this computer. No port forwarding needed.',
  'server.share.start': 'Share',
  'server.share.state.starting': 'Starting…',
  'server.share.state.sharing': 'Sharing',
  'server.share.state.reconnecting': 'Reconnecting…',
  'server.share.stableLink': 'The link keeps working after a restart.',
  'server.share.linkLabel': 'Invite link',
  'server.share.copy': 'Copy link',
  'server.share.copied': 'Copied the invite link',
  'server.share.copyFailed': "Couldn't copy. Select the link and copy it.",
  'server.share.preparing': 'Getting the link…',
  'server.share.howToJoin':
    'Anyone with the link can join this server while it runs. People without Lodestone Launcher get a download link.',
  'server.share.hardNetwork':
    "This network's router makes direct connections hard. Some people may not be able to join.",
  'server.share.connected': {
    one: '{count} person connected',
    other: '{count} people connected',
  },
  'server.share.joins': {
    one: '({count} join so far)',
    other: '({count} joins so far)',
  },
  /** {connection} is one of server.share.connection.*, {ms} the round trip time */
  'server.share.peerLine': 'over {connection}, {ms} ms',
  'server.share.connection.same_computer': 'this computer',
  'server.share.connection.local_network': 'the local network',
  'server.share.connection.ipv6': 'IPv6',
  'server.share.connection.internet': 'the internet',
  'server.share.stop': 'Stop sharing',
  'server.share.stopTitle': 'Stop sharing?',
  'server.share.stopBody':
    'Everyone playing through the link is disconnected, and the link stops working. Sharing again makes a new link.',
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
  /** copies the server's address (servers that can't be shared) */
  'server.card.copyIp': 'Copy IP',
  /** opens the Overview's Share section */
  'server.card.share': 'Share',
  'server.card.shareHint': 'Share this server with a link',
  /** online players / maximum, e.g. "3/20 Online" */
  'server.card.online': '{players}/{max} Online',

  // InstanceCard: starting a server whose Minecraft EULA isn't accepted
  'server.eula.title': 'Accept the Minecraft EULA',
  /** {link} is create.eula.link, a link to the EULA; {name} is the server's name */
  'server.eula.body':
    '{name} only starts after you agree to the {link}. Do you agree to it?',
  /** the confirm button: accepts the EULA and starts the server */
  'server.eula.accept': 'I agree, start the server',
  /** {error}: the core's error message */
  'server.eula.acceptFailed': 'Could not accept the Minecraft EULA: {error}',

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

  // ===== Console =====

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

  // CodeExecution: the custom Java command settings and the code execution switch
  /** why the Java command / arguments settings can't be changed */
  'console.commandLine.readOnly':
    'Read-only while code execution is disabled on this core.',
  'console.commandLine.ownerOnly': 'Only the owner can change this setting.',

  // ===== Creating a server =====

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
  /** the required box before creating a Minecraft server; {link} is create.eula.link, a link to the EULA */
  'create.eula.agree': 'I agree to the {link}',
  /** the text of the link to Mojang's EULA */
  'create.eula.link': 'Minecraft EULA',
  /** under the box while it is not ticked */
  'create.eula.required':
    'A Minecraft server only runs after you agree to the Minecraft EULA.',

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
  /** a badge next to the Modrinth modpack card's title */
  'create.gameType.new': 'New',
  'create.gameType.modpack': 'Modrinth modpack',
  'create.gameType.modpackDescription':
    'Create a Fabric, Forge or NeoForge server from a Modrinth modpack',

  // GameTypeMappings: games' titles and descriptions
  /** {name}: the game's name, as the core sends it */
  'create.game.genericTitle': '{name} (Generic)',
  'create.game.vanillaDescription':
    'Standard vanilla Minecraft server from Mojang.',
  'create.game.forgeDescription':
    'Modding framework that allows you to install mods and customize your Minecraft experience.',
  'create.game.neoforgeDescription':
    'Community fork of Forge with its own mods, for Minecraft 1.20.2 and later.',
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
    'Search Modrinth, paste a Modrinth link, or upload a .mrpack file. Fabric, Forge and NeoForge modpacks are supported.',
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

  // ===== File manager =====

  // FileViewer: the page, its toolbar and dialogs
  /** the page title, inside common.pageTitle */
  'files.pageTitle': 'Server File Viewer',
  /** shown while files are dragged over the file viewer */
  'files.dropToUpload': 'Release to Upload File',
  /** the toolbar menu with the actions below */
  'files.toolbar.addRemove': 'Add/Remove',
  'files.toolbar.selectAll': 'Select all',
  'files.toolbar.deselectAll': 'Deselect all',
  'files.toolbar.cutSelected': 'Cut selected',
  'files.toolbar.downloadSelected': 'Download selected',
  'files.toolbar.unarchiveSelected': 'Unarchive selected',
  'files.toolbar.deleteSelected': 'Delete selected',
  /** pastes the files in the clipboard into the folder being shown */
  'files.toolbar.paste': {
    one: 'Paste {count} file',
    other: 'Paste {count} files',
  },
  /** reverts the edits in the file editor */
  'files.toolbar.discardChanges': 'Discard Changes',
  'files.toolbar.upload': 'Upload',
  /** a toast after “Cut selected” */
  'files.cutToClipboard': {
    one: '{count} file cut to clipboard',
    other: '{count} files cut to clipboard',
  },
  /** a toast: a folder is compressed before it is downloaded */
  'files.zippingForDownload': 'Zipping directory for download…',
  /** {name} is a file name, {error} the reason */
  'files.downloadFailed': 'Failed to download {name}: {error}',
  'files.onlyFilesUnzip': 'Only files can be unzipped',
  /** a toast when a folder is dropped to upload it */
  'files.onlyFilesUpload': 'Only files can be uploaded',
  /** the title of the dialog confirming a deletion */
  'files.delete.title': { one: 'Delete file', other: 'Delete {count} files' },
  /** followed by the list of files and folders */
  'files.delete.message': 'Are you sure you want to delete the following?',
  /** after a folder’s name in the list */
  'files.delete.directory': '(Directory)',
  /** after the name of an entry that is neither a file nor a folder */
  'files.delete.unknownType': '(Unknown)',
  /** for screen readers, while a file loads */
  'files.loadingFile': 'Loading file',
  /** shown in the editor pane when no file can be shown */
  'files.editor.title': 'File Editor',
  'files.editor.selectFile': 'Select a file to view its contents',
  'files.noReadPermission': 'You don’t have permission to read this folder',
  /** below the file list: files and folders selected */
  'files.status.selected': {
    one: '{count} item selected',
    other: '{count} items selected',
  },
  /** below the file list: files and folders copied or cut */
  'files.status.inClipboard': {
    one: '{count} item in clipboard',
    other: '{count} items in clipboard',
  },

  // FileList: the list of files and folders
  /** for screen readers, while a folder loads */
  'files.list.loading': 'Loading files',
  'files.list.empty': 'No files here…',
  /** in the modification time column */
  'files.list.unknownTime': 'Unknown Creation Time',
  /** alt text of an icon */
  'files.list.folderIcon': 'folder icon',
  /** alt text of an icon */
  'files.list.fileIcon': 'file icon',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': 'Cut',
  'files.contextMenu.paste': 'Paste',
  /** {name} is a folder name */
  'files.contextMenu.pasteInto': 'Paste into {name}',
  /** compresses files into a .zip archive */
  'files.contextMenu.zip': 'Zip',
  /** {archive} is the archive’s file name, e.g. “world.zip” */
  'files.contextMenu.zipTo': 'Zip to {archive}',
  /** {name} is a file name, {archive} the archive’s file name */
  'files.contextMenu.zipFile': 'Zip {name} to {archive}',
  /** {archive} is the archive’s file name, “Archive.zip” */
  'files.contextMenu.zipFiles': {
    one: 'Zip {count} file to {archive}',
    other: 'Zip {count} files to {archive}',
  },
  /** extracts the archive into the folder being shown */
  'files.contextMenu.unzipHere': 'Unzip here',
  /** extracts the archive, without its single top-level folder if it has one */
  'files.contextMenu.unzipHereSmart': 'Unzip here (smart)',
  /** {name} is the folder the archive is extracted to (its name without the extension) */
  'files.contextMenu.unzipTo': 'Unzip to {name}',
  /** a toast after Copy: files and folders */
  'files.contextMenu.copiedToClipboard': {
    one: '{count} item copied to clipboard',
    other: '{count} items copied to clipboard',
  },
  /** a toast after Cut: files and folders */
  'files.contextMenu.cutToClipboard': {
    one: '{count} item cut to clipboard',
    other: '{count} items cut to clipboard',
  },
  'files.contextMenu.unsupportedZip': 'Unsupported zip file type',
  /** in the toolbar menu and the right-click menu */
  'files.newFolder': 'New folder',
  /** in the toolbar menu and the right-click menu */
  'files.newFile': 'New file',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': 'Name your file',
  /** the placeholder of the file name field */
  'files.createFile.placeholder': 'Untitled',
  'files.createFile.submit': 'Create file',
  'files.createFolder.label': 'Name your folder',
  /** the placeholder of the folder name field */
  'files.createFolder.placeholder': 'Untitled folder',
  'files.createFolder.submit': 'Create folder',
  /** New file or New folder with the name of something already in the folder */
  'files.nameTaken': '{name} already exists in this folder',
  /** the new name of a file or folder */
  'files.rename.label': 'Name your file',
  'files.rename.submit': 'Rename file',

  // FileEditor
  /** for screen readers, while the code editor starts */
  'files.editor.loading': 'Loading editor',

  // ===== Mods =====

  // The Mods (or Plugins) tab of a server: installed mods, Modrinth search and
  // project view, client modpack. A server has either mods (Fabric, Forge) or
  // plugins (Paper, Spigot); messages that name them have a .mod and a .plugin key.

  // utils/mods: the tab's name in the server's tab bar and side bar
  'mods.tab.mods': 'Mods',
  'mods.tab.plugins': 'Plugins',

  // ModsTab
  /** the browser tab's title for the Mods tab */
  'mods.pageTitle.mod': 'Server Mods',
  'mods.pageTitle.plugin': 'Server Plugins',
  'mods.unsupported':
    'This server does not support mods or plugins. Fabric, Forge, Paper and Spigot servers do.',
  'mods.noReadPermission':
    'You don’t have permission to read this instance’s files.',
  /** the sub-view switch: the installed mods, and Modrinth browsing */
  'mods.view.installed': 'Installed',
  'mods.view.browse': 'Browse',
  'mods.restartRequired.mod': 'Restart the server to apply mod changes',
  'mods.restartRequired.plugin': 'Restart the server to apply plugin changes',
  'mods.browse.placeholder.mod':
    'Search mods on Modrinth, or paste a Modrinth link',
  'mods.browse.placeholder.plugin':
    'Search plugins on Modrinth, or paste a Modrinth link',
  /** a checkbox; e.g. "Only show versions for 1.21.1 / fabric" (loader as Modrinth names it) */
  'mods.browse.onlyCompatible':
    'Only show versions for {gameVersion} / {loader}',

  // common: labels, tooltips and the install toast shared by the views
  /** when the user can't change the server's files (a tooltip on disabled buttons) */
  'mods.noWritePermission':
    'You don’t have permission to change this server’s files',
  /** on a Modrinth project that players must install too */
  'mods.clientSide.playersNeedToo': 'Players need this too',
  /** on a project or mod that only runs on the server */
  'mods.clientSide.serverOnly': 'Server only',
  /** on an installed mod that players need to join */
  'mods.clientRequirement.required': 'Players need this',
  /** {mods}: a list of mod names */
  'mods.clientRequirement.neededBy': 'Needed by {mods}',
  'mods.clientRequirement.unknownModrinth':
    'Modrinth doesn’t say whether players need it; check the mod’s page',
  'mods.clientRequirement.unknownNotOnModrinth':
    'Not on Modrinth; check the mod’s page',
  /** with the "For players" box (Fabric servers) */
  'mods.clientRequirement.unknownModrinthMark':
    'Modrinth doesn’t say whether players need it. Check the mod’s page, and tick For players if they do.',
  'mods.clientRequirement.unknownNotOnModrinthMark':
    'Not on Modrinth. Check the mod’s page, and tick For players if players need it.',
  /** a checkbox on a Fabric server's mod row: players download the mod too */
  'mods.forPlayers.label': 'For players',
  'mods.forPlayers.hint':
    'Players get this mod, and the mods it needs, when they join. Tick it for mods players need that Lodestone can’t tell, such as ones you uploaded.',
  'mods.forPlayers.already':
    'Players get this mod already: Modrinth or the mod says they need it.',
  'mods.forPlayers.optional':
    'Modrinth says players can join without this mod, but on Fabric mods like it often stop them joining, so players get it. Untick it to leave it out.',
  /** toasts after ticking or unticking the box; {name}: the mod's name */
  'mods.forPlayers.marked': 'Players get {name} the next time they join',
  'mods.forPlayers.unmarked': 'Players no longer get {name} when they join',
  'mods.forPlayers.failed': 'Couldn’t change {name}: {error}',
  /** a Modrinth version's type, on a small label */
  'mods.versionType.release': 'release',
  'mods.versionType.beta': 'beta',
  'mods.versionType.alpha': 'alpha',
  /** in the toast after an install; the lists are mod names and versions */
  'mods.installResult.installed': 'Installed: {mods}',
  'mods.installResult.removed': 'Removed: {files}',
  'mods.installResult.skipped': 'Skipped: {projects}',
  'mods.installResult.playersNeed': 'Players will need {mods} too',
  /** a project's author on a search result and the project page */
  'mods.byAuthor': 'by {author}',
  /** on a search result or version that is installed on the server */
  'mods.installedLabel': 'Installed',
  /** a tooltip on a project's download count */
  'mods.downloads': { one: '{count} download', other: '{count} downloads' },
  'mods.followers': { one: '{count} follower', other: '{count} followers' },

  // ModrinthSearch
  'mods.sort.label': 'Sort by',
  'mods.sort.relevance': 'Relevance',
  'mods.sort.downloads': 'Downloads',
  'mods.sort.follows': 'Follows',
  'mods.sort.newest': 'Newest',
  'mods.sort.updated': 'Recently updated',
  'mods.search.ariaLabel': 'Search Modrinth',
  /** inside the search box while a pasted modrinth.com link resolves */
  'mods.search.opening': 'Opening the link…',
  'mods.search.linkFailed': 'Couldn’t open the link: {error}',
  /** {slug}: the project's short name in the link */
  'mods.search.openLink': 'Open {slug}',
  'mods.search.failed': 'Search failed: {error}',
  'mods.search.loading': 'Loading results',
  'mods.search.noResults': 'No results.',
  'mods.search.page': 'Page {page} of {total}',
  'mods.search.previousPage': 'Previous page',
  'mods.search.nextPage': 'Next page',

  // InstalledMods
  'mods.installed.enableFailed': 'Failed to enable {name}: {error}',
  'mods.installed.disableFailed': 'Failed to disable {name}: {error}',
  'mods.installed.updated': 'Updated {name} to {version}',
  'mods.installed.updateFailed': 'Failed to update {name}: {error}',
  /** a button; {version}: the new version's name */
  'mods.installed.updateTo': 'Update to {version}',
  'mods.installed.viewOnModrinth': 'View on Modrinth',
  /** the aria-label of a mod's delete button */
  'mods.installed.deleteLabel': 'Delete {name}',
  'mods.installed.uploaded': 'Uploaded {names}',
  'mods.installed.uploadFailed': 'Upload failed: {error}',
  'mods.installed.onlyJars': 'Only .jar files can be uploaded: {names}',
  'mods.installed.updatesInstalled': 'Updates installed',
  /** {failures}: "name: error" for each failed update */
  'mods.installed.someUpdatesFailed': 'Some updates failed: {failures}',
  'mods.installed.deleted': 'Deleted {name}',
  'mods.installed.deleteFailed': 'Failed to delete: {error}',
  'mods.installed.deleteTitle.mod': 'Delete mod?',
  'mods.installed.deleteTitle.plugin': 'Delete plugin?',
  /** {name}: the mod's name, {fileName}: its jar file */
  'mods.installed.deleteBody':
    '{name} ({fileName}) will be deleted from the server. This can’t be undone.',
  'mods.installed.overwriteTitle': 'Overwrite existing files?',
  'mods.installed.overwrite': 'Overwrite',
  /** {message}: the core's message naming the file, or mods.installed.overwriteDefault */
  'mods.installed.overwriteBody':
    '{message} Replace it with the uploaded file?',
  'mods.installed.overwriteDefault':
    'A file with the same name is already installed.',
  /** above the list; {directory}: the folder, e.g. "mods" */
  'mods.installed.count.mod': {
    one: '{count} mod in {directory}/',
    other: '{count} mods in {directory}/',
  },
  'mods.installed.count.plugin': {
    one: '{count} plugin in {directory}/',
    other: '{count} plugins in {directory}/',
  },
  'mods.installed.updateAll': 'Update all ({count})',
  'mods.installed.checkUpdates': 'Check for updates',
  'mods.installed.upload': 'Upload',
  'mods.installed.loadFailed.mod': 'Failed to load mods: {error}',
  'mods.installed.loadFailed.plugin': 'Failed to load plugins: {error}',
  'mods.installed.loading': 'Loading mods',
  'mods.installed.empty.mod':
    'No mods installed yet. Browse Modrinth or upload a .jar file.',
  'mods.installed.empty.plugin':
    'No plugins installed yet. Browse Modrinth or upload a .jar file.',

  // ModpackPanel: a server's modpack, its updates and history
  'mods.modpack.from': 'This server runs the modpack {pack}.',
  'mods.modpack.check': 'Check for an update',
  'mods.modpack.checkFailed': 'Couldn’t check for an update: {error}',
  'mods.modpack.upToDate': 'It’s the newest version.',
  'mods.modpack.whatsNew': 'What’s new',
  'mods.modpack.out': '{version} is out',
  'mods.modpack.updateTo': 'Update to {version}',
  'mods.modpack.fromFile': 'Update from a file',
  'mods.modpack.notMrpack': '{name} isn’t a .mrpack file.',
  'mods.modpack.planFailed': 'Couldn’t prepare the update: {error}',
  'mods.modpack.history': 'History',
  'mods.modpack.historyFailed': 'Couldn’t load the modpack’s history: {error}',
  'mods.modpack.historyEmpty':
    'Nothing is recorded yet. Updates from now on can be undone.',
  'mods.modpack.kind.install': 'Installed',
  'mods.modpack.kind.baseline': 'First recorded',
  'mods.modpack.kind.update': 'Updated',
  'mods.modpack.kind.restore': 'Restored {version}',
  'mods.modpack.current': 'Current',
  'mods.modpack.restore': 'Restore',
  'mods.modpack.restoreMissing':
    'Lodestone no longer has some of this version’s files.',
  'mods.modpack.restoreTitle': 'Restore this version?',
  'mods.modpack.updateTitle': 'Update the modpack?',
  'mods.modpack.update': 'Update',
  'mods.modpack.restored': 'Restored {pack} {version}.',
  'mods.modpack.updated': 'Updated {pack} to {version}.',
  'mods.modpack.restoreFailed': 'Couldn’t restore the modpack: {error}',
  'mods.modpack.updateFailed': 'Couldn’t update the modpack: {error}',
  'mods.modpack.storeSize':
    'To undo updates, Lodestone keeps {size} of files that can’t be downloaded again.',
  'mods.modpack.since': 'since {version}',
  'mods.modpack.sinceTooltip': 'This file came with {version} of the modpack.',
  'mods.modpack.unknownVersion': 'unknown version',
  'mods.modpack.dialog.intro': '{pack} goes from {from} to {to}.',
  'mods.modpack.dialog.reinstall':
    'The server moves to {software} and installs it again.',
  'mods.modpack.dialog.kept': 'Files you changed stay as they are: {files}',
  'mods.modpack.dialog.nothing': 'No files change.',
  'mods.modpack.dialog.stop':
    'Stop the server first: it can’t be updated while it runs.',
  'mods.modpack.dialog.unsupported': 'This version can’t be used: {reason}',
  'mods.modpack.added': {
    one: '{count} file added',
    other: '{count} files added',
  },
  'mods.modpack.replaced': {
    one: '{count} file replaced',
    other: '{count} files replaced',
  },
  'mods.modpack.removed': {
    one: '{count} file removed',
    other: '{count} files removed',
  },
  'mods.modpack.keptCount': {
    one: '{count} changed file kept',
    other: '{count} changed files kept',
  },
  'mods.modpack.moreChanges': {
    one: 'and {count} more',
    other: 'and {count} more',
  },

  // ClientPackPanel: the mods players need on a Fabric server
  /** {version}: the Fabric loader version, e.g. 0.16.5 */
  'mods.clientPack.loader': 'Fabric loader {version}',
  'mods.clientPack.loaderNoVersion': 'Fabric loader',
  /** {loader}: mods.clientPack.loader */
  'mods.clientPack.plainFabric':
    'Players can join with plain Fabric (Minecraft {gameVersion}, {loader}).',
  'mods.clientPack.title': {
    one: 'Players need {count} mod to join',
    other: 'Players need {count} mods to join',
  },
  'mods.clientPack.download': 'Download client modpack (.mrpack)',
  /** Under the first few of a big client pack's mods: shows them all */
  'mods.clientPack.showAll': {
    one: 'Show all {count} mod',
    other: 'Show all {count} mods',
  },
  /** Folds the client pack's mods again, after mods.clientPack.showAll */
  'mods.clientPack.showFewer': 'Show fewer',
  'mods.clientPack.downloadFailed':
    'Failed to download the client modpack: {error}',
  'mods.clientPack.importHint':
    'Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions.',
  /** mods.clientPack.importHint, when some mods aren't on Modrinth */
  'mods.clientPack.importHintWithJars': {
    one: 'Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions. {count} mod isn’t on Modrinth, so its file is included in the pack.',
    other:
      'Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions. {count} mods aren’t on Modrinth, so their files are included in the pack.',
  },

  // ModrinthProjectView
  'mods.project.loading': 'Loading project',
  'mods.project.loadFailed': 'Failed to load the project: {error}',
  /** links on a project page, next to "Modrinth" and "Discord" */
  'mods.project.link.source': 'Source',
  'mods.project.link.issues': 'Issues',
  'mods.project.link.wiki': 'Wiki',
  /** whether the project runs on servers; {value}: mods.project.side.* */
  'mods.project.serverSide': 'server: {value}',
  'mods.project.side.required': 'required',
  'mods.project.side.optional': 'optional',
  'mods.project.side.unsupported': 'unsupported',
  'mods.project.side.unknown': 'unknown',
  'mods.project.createFromModpack': 'Create a server from this modpack',
  'mods.project.modpackNoPermission':
    'This is a modpack. You don’t have permission to create servers.',
  'mods.project.noServerSupport':
    'This project says it doesn’t run on servers.',
  /** {type}: the Modrinth project type, e.g. "resourcepack" */
  'mods.project.wrongType.mod':
    'This is a {type}. Lodestone only installs mods here.',
  'mods.project.wrongType.plugin':
    'This is a {type}. Lodestone only installs plugins here.',
  'mods.project.description': 'Description',
  'mods.versions.title': 'Versions',
  'mods.versions.withDependencies': 'Also install required dependencies',
  'mods.versions.showAll': 'Show all versions',
  'mods.versions.loadFailed': 'Failed to load versions: {error}',
  'mods.versions.loading': 'Loading versions',
  /** {loader}: e.g. fabric, {gameVersion}: e.g. 1.21.1 */
  'mods.versions.none': 'No versions for {loader} {gameVersion}.',
  'mods.versions.noneShowAll':
    'No versions for {loader} {gameVersion}. Turn on “Show all versions” to see the others.',
  'mods.versions.showMore': {
    one: 'Show more ({count} left)',
    other: 'Show more ({count} left)',
  },
  /** after the first 3 game versions of a version, e.g. "1.20, 1.20.1, 1.20.2 +5 more" */
  'mods.version.moreGameVersions': {
    one: '+{count} more',
    other: '+{count} more',
  },
  /** on a version for another loader or Minecraft version, e.g. "Not for fabric 1.21.1" */
  'mods.version.notFor': 'Not for {loader} {gameVersion}',
  'mods.version.install': 'Install',
  /** a toast title; {title}: the project, {version}: the version's name */
  'mods.version.installed': 'Installed {title} {version}',
  'mods.version.installFailed': 'Failed to install {title}: {error}',
  'mods.version.incompatibleTitle': 'Install an incompatible version?',
  'mods.version.installAnyway': 'Install anyway',
  /** {loaders} and {gameVersions}: lists from Modrinth; {loader} {gameVersion}: the server's */
  'mods.version.incompatibleBody':
    '{title} {version} is made for {loaders} {gameVersions}, not {loader} {gameVersion}. It may crash the server.',
} as const;
