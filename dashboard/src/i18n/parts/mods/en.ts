// The Mods (or Plugins) tab of a server: installed mods, Modrinth search and
// project view, client modpack. A server has either mods (Fabric, Forge) or
// plugins (Paper, Spigot); messages that name them have a .mod and a .plugin key.
export default {
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
