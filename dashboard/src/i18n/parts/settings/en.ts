export default {
  // Profile → Appearance → Language
  'settings.language.label': 'Language',
  'settings.language.hint':
    'The language of the dashboard. Names, logs and messages from servers and the core stay as they are.',
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
  'settings.core.description': 'These settings are for the core itself.',
  'settings.core.section.core': 'Core',
  'settings.core.section.public': 'Public servers',
  'settings.core.section.publicLead':
    'What anyone can see without signing in. Only you can change this.',
  'settings.core.section.info': 'Information',
  'settings.core.section.infoLead':
    'This is information about your core and dashboard.',
  'settings.core.section.danger': 'Danger zone',
  'settings.core.section.dangerLead':
    'These settings can cause irreversible damage to your core.',
  'settings.core.name.label': 'Core name',
  'settings.core.name.hint':
    'A nickname for this core. This is what you and others will see when you connect to this core',
  'settings.core.name.empty': 'Name cannot be empty',
  'settings.core.name.tooLong': 'Name cannot be longer than {max} characters',
  'settings.core.domain.label': 'Public domain or IP',
  'settings.core.domain.hint': 'The domain or public IP address of this core',
  'settings.core.domain.tooLong':
    'Domain cannot be longer than {max} characters',
  'settings.core.domain.tooltip':
    'This is your public IP if you are port-forwarded. If not, leave this as is.',
  'settings.core.playit.label': 'Enable Playit integration',
  'settings.core.playit.hint':
    'Enable Playit integration allowing users to open their servers for public facing access without port forwarding',
  'settings.core.listPublicly.label': 'List servers publicly',
  'settings.core.listPublicly.hint':
    'Let anyone who can reach this core see the servers you make public below, without signing in, and only what you choose for each. Turning this off hides them all at once. Lodestone Launcher uses this to join them.',
  'settings.core.safeMode.label': 'Safe mode',
  'settings.core.safeMode.hint':
    'Attempts to prevent non-owner users from accessing administrative permissions on your machine',
  'settings.core.safeMode.offTitle': 'Turn off safe mode?',
  'settings.core.safeMode.offBody':
    'Are you sure you want to turn off safe mode? This will allow you to give users other than yourself the ability to run potentially dangerous commands. Make sure you trust all the users you give these permissions to.',
  /** the confirm button of the "Turn off safe mode / code execution?" dialogs */
  'settings.core.turnOff': 'Turn off',
  'settings.core.codeExecution.label': 'Code execution',
  'settings.core.codeExecution.hint':
    'Allow macros, atom servers, extensions, server scripts and custom Java commands to run on this core.',
  /** the same hint on a core without scripting (Deno) */
  'settings.core.codeExecution.hintJavaOnly':
    'Allow custom Java commands to run on this core.',
  'settings.core.codeExecution.locked':
    'Locked by this core’s administrator (LODESTONE_DISABLE_CODE_EXECUTION or --disable-code-execution).',
  'settings.core.codeExecution.offTitle': 'Turn off code execution?',
  'settings.core.codeExecution.offBody':
    'Running macros and atom servers will be stopped. Macros, atom servers, extensions, server scripts and custom Java commands will not run on this core until you turn code execution back on. Nothing is started again automatically.',
  /** the same dialog on a core without scripting (Deno) */
  'settings.core.codeExecution.offBodyJavaOnly':
    'Custom Java commands will not run on this core until you turn code execution back on. Nothing is started again automatically.',
  'settings.core.lanHttp.label': 'Allow LAN access over plain HTTP',
  'settings.core.lanHttp.hint':
    'Let other devices connect with http:// instead of https://. Passwords and session tokens then cross your network unencrypted. This machine can always use http://.',
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
  'settings.core.openPort.hint':
    'Attempt to port forward on your router using {upnp}. This will allow people on the internet to connect to your server.',
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
  'settings.users.description':
    'The accounts on this core and what each one can do.',
  'settings.users.allUsers': 'All users',
  'settings.users.allMembers': 'All members',
  'settings.users.allMembersCount': 'All members ({count})',
  'settings.users.allMembersLead':
    'A list of all users. Click a user to manage them.',
  'settings.users.createUser': 'Create new user',
  'settings.users.createTitle': 'Create a new user account',
  'settings.users.createLead':
    'This user will start with no permissions. You can grant them permissions later.',
  'settings.users.loadingUsers': 'Loading users',
  'settings.users.loadingUser': 'Loading the user',
  'settings.users.loadingServers': 'Loading servers',
  'settings.users.permissions': 'Permissions',
  'settings.users.permissionsLead': 'What {username} can do on this core.',
  'settings.users.unsafePermissions': 'Unsafe permissions',
  'settings.users.unsafePermissionsLead':
    'Turn off safe mode in core settings to grant these permissions to non-owner users.',
  'settings.users.ownerOnly': 'Macros, extensions and atom servers',
  'settings.users.ownerOnlyHint':
    'Only the owner can use macros, install extensions, and create or control atom servers. These can’t be granted to other users. Docker servers use the normal server permissions.',

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
  'settings.profile.description': 'Profile preferences for {username}',
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
  'settings.profile.coloredConsoleHint':
    'Shows the colors and styles servers print in the server console',

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
  'settings.minecraft.ownLead':
    'Accounts you linked from Lodestone Launcher, which checks with Microsoft that they are yours. The dashboard shows your name next to them.',
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
    'Keys let the CLI, scripts and agents use Lodestone as you, without your password. Revoking a key doesn’t sign you out anywhere else.',
  'settings.apiKeys.ownEmpty': 'You have no API keys.',
  'settings.apiKeys.userLead':
    'The keys {username} created. Revoke a key to cut off what uses it at once. Only {username} can create keys for themselves.',
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
    'The CLI, scripts and agents can sign in with an API key instead of your password. Give each key a name, a short expiry and only the access it needs.',
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
  'settings.apiKeys.ownerOnly': 'Macros, Extensions and Atom Servers',
  'settings.apiKeys.ownerOnlyHint':
    'Keys with custom access can’t use macros, install extensions, or create or control atom servers, even if they belong to the owner. A key with full access can, if you can.',
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
} as const;
