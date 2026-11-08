export default {
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
  'shell.event.playerMessage': '{player} said {message} on {name}',
  /** {name} is the name of an API key */
  'shell.event.apiKeyCreated': 'API key “{name}” created',
  'shell.event.apiKeyRevoked': 'API key “{name}” revoked',
  /** {pid} is a macro's process id, {server} a server's id or "Core" */
  'shell.event.macroStarted': 'Macro {pid} started on {server}',
  'shell.event.macroDetached': 'Macro {pid} detached on {server}',
  /** {status} is a technical exit status, e.g. "Success" */
  'shell.event.macroStopped':
    'Macro {pid} stopped on {server} with status {status}',
  'shell.event.playitStarted': 'Playitgg runner started!',
  'shell.event.playitLoading': 'Playitgg runner loading…',
  'shell.event.playitStopped': 'Playitgg runner stopped!',
  /** {error} is an error message */
  'shell.event.loadFailed': 'Failed to load notifications: {error}',

  // SystemInfo
  'shell.noScripting':
    'This core was built without scripting support (Deno), so macros aren’t available.',

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

  // CoreConnect: adding a core
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
} as const;
