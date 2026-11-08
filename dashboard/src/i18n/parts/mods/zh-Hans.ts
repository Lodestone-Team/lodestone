import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // utils/mods
  'mods.tab.mods': '模组',
  'mods.tab.plugins': '插件',

  // ModsTab
  'mods.pageTitle.mod': '服务器模组',
  'mods.pageTitle.plugin': '服务器插件',
  'mods.unsupported':
    '此服务器不支持模组或插件。Fabric、Forge、Paper 和 Spigot 服务器支持。',
  'mods.noReadPermission': '你没有读取此实例文件的权限。',
  'mods.view.installed': '已安装',
  'mods.view.browse': '浏览',
  'mods.restartRequired.mod': '重启服务器以应用模组更改',
  'mods.restartRequired.plugin': '重启服务器以应用插件更改',
  'mods.browse.placeholder.mod': '在 Modrinth 上搜索模组，或粘贴 Modrinth 链接',
  'mods.browse.placeholder.plugin':
    '在 Modrinth 上搜索插件，或粘贴 Modrinth 链接',
  'mods.browse.onlyCompatible': '仅显示适用于 {gameVersion} / {loader} 的版本',

  // common
  'mods.noWritePermission': '你没有更改此服务器文件的权限',
  'mods.clientSide.playersNeedToo': '玩家也需要安装',
  'mods.clientSide.serverOnly': '仅服务器',
  'mods.clientRequirement.required': '玩家需要安装',
  'mods.clientRequirement.neededBy': '{mods} 需要它',
  'mods.clientRequirement.unknownModrinth':
    'Modrinth 未说明玩家是否需要它，请查看该模组的页面',
  'mods.clientRequirement.unknownNotOnModrinth':
    '不在 Modrinth 上，请查看该模组的页面',
  'mods.versionType.release': '正式版',
  'mods.versionType.beta': '测试版',
  'mods.versionType.alpha': '早期测试版',
  'mods.installResult.installed': '已安装：{mods}',
  'mods.installResult.removed': '已移除：{files}',
  'mods.installResult.skipped': '已跳过：{projects}',
  'mods.installResult.playersNeed': '玩家也需要安装 {mods}',
  'mods.byAuthor': '作者：{author}',
  'mods.installedLabel': '已安装',
  'mods.downloads': { other: '{count} 次下载' },
  'mods.followers': { other: '{count} 位关注者' },

  // ModrinthSearch
  'mods.sort.label': '排序方式',
  'mods.sort.relevance': '相关性',
  'mods.sort.downloads': '下载量',
  'mods.sort.follows': '关注数',
  'mods.sort.newest': '最新发布',
  'mods.sort.updated': '最近更新',
  'mods.search.ariaLabel': '搜索 Modrinth',
  'mods.search.opening': '正在打开链接…',
  'mods.search.linkFailed': '无法打开链接：{error}',
  'mods.search.openLink': '打开 {slug}',
  'mods.search.failed': '搜索失败：{error}',
  'mods.search.loading': '正在加载结果',
  'mods.search.noResults': '没有结果。',
  'mods.search.page': '第 {page} 页，共 {total} 页',
  'mods.search.previousPage': '上一页',
  'mods.search.nextPage': '下一页',

  // InstalledMods
  'mods.installed.enableFailed': '无法启用 {name}：{error}',
  'mods.installed.disableFailed': '无法禁用 {name}：{error}',
  'mods.installed.updated': '已将 {name} 更新到 {version}',
  'mods.installed.updateFailed': '无法更新 {name}：{error}',
  'mods.installed.updateTo': '更新到 {version}',
  'mods.installed.viewOnModrinth': '在 Modrinth 上查看',
  'mods.installed.deleteLabel': '删除 {name}',
  'mods.installed.uploaded': '已上传 {names}',
  'mods.installed.uploadFailed': '上传失败：{error}',
  'mods.installed.onlyJars': '只能上传 .jar 文件：{names}',
  'mods.installed.updatesInstalled': '已安装更新',
  'mods.installed.someUpdatesFailed': '部分更新失败：{failures}',
  'mods.installed.deleted': '已删除 {name}',
  'mods.installed.deleteFailed': '删除失败：{error}',
  'mods.installed.deleteTitle.mod': '删除模组？',
  'mods.installed.deleteTitle.plugin': '删除插件？',
  'mods.installed.deleteBody':
    '{name}（{fileName}）将从服务器中删除。此操作无法撤销。',
  'mods.installed.overwriteTitle': '覆盖现有文件？',
  'mods.installed.overwrite': '覆盖',
  'mods.installed.overwriteBody': '{message}要用上传的文件替换它吗？',
  'mods.installed.overwriteDefault': '已安装同名文件。',
  'mods.installed.count.mod': { other: '{directory}/ 中有 {count} 个模组' },
  'mods.installed.count.plugin': { other: '{directory}/ 中有 {count} 个插件' },
  'mods.installed.updateAll': '全部更新（{count}）',
  'mods.installed.checkUpdates': '检查更新',
  'mods.installed.upload': '上传',
  'mods.installed.loadFailed.mod': '无法加载模组：{error}',
  'mods.installed.loadFailed.plugin': '无法加载插件：{error}',
  'mods.installed.loading': '正在加载模组',
  'mods.installed.empty.mod':
    '尚未安装任何模组。浏览 Modrinth 或上传 .jar 文件。',
  'mods.installed.empty.plugin':
    '尚未安装任何插件。浏览 Modrinth 或上传 .jar 文件。',

  // ClientPackPanel
  'mods.clientPack.loader': 'Fabric Loader {version}',
  'mods.clientPack.loaderNoVersion': 'Fabric Loader',
  'mods.clientPack.plainFabric':
    '玩家只需原版 Fabric 即可加入（Minecraft {gameVersion}，{loader}）。',
  'mods.clientPack.title': { other: '玩家需要 {count} 个模组才能加入' },
  'mods.clientPack.download': '下载客户端整合包（.mrpack）',
  'mods.clientPack.downloadFailed': '无法下载客户端整合包：{error}',
  'mods.clientPack.importHint':
    '将其导入 Prism Launcher、Modrinth App 或 ATLauncher，即可获得所有正确版本的内容。',
  'mods.clientPack.importHintWithJars': {
    other:
      '将其导入 Prism Launcher、Modrinth App 或 ATLauncher，即可获得所有正确版本的内容。有 {count} 个模组不在 Modrinth 上，因此其文件已包含在整合包中。',
  },

  // ModrinthProjectView
  'mods.project.loading': '正在加载项目',
  'mods.project.loadFailed': '无法加载项目：{error}',
  'mods.project.link.source': '源代码',
  'mods.project.link.issues': '问题反馈',
  'mods.project.link.wiki': 'Wiki',
  'mods.project.serverSide': '服务器：{value}',
  'mods.project.side.required': '必需',
  'mods.project.side.optional': '可选',
  'mods.project.side.unsupported': '不支持',
  'mods.project.side.unknown': '未知',
  'mods.project.createFromModpack': '用此整合包创建服务器',
  'mods.project.modpackNoPermission':
    '这是一个整合包。你没有创建服务器的权限。',
  'mods.project.noServerSupport': '此项目声明它无法在服务器上运行。',
  'mods.project.wrongType.mod':
    '这是一个 {type} 项目。Lodestone 在这里只安装模组。',
  'mods.project.wrongType.plugin':
    '这是一个 {type} 项目。Lodestone 在这里只安装插件。',
  'mods.project.description': '描述',
  'mods.versions.title': '版本',
  'mods.versions.withDependencies': '同时安装必需的依赖项',
  'mods.versions.showAll': '显示所有版本',
  'mods.versions.loadFailed': '无法加载版本：{error}',
  'mods.versions.loading': '正在加载版本',
  'mods.versions.none': '没有适用于 {loader} {gameVersion} 的版本。',
  'mods.versions.noneShowAll':
    '没有适用于 {loader} {gameVersion} 的版本。打开“显示所有版本”以查看其他版本。',
  'mods.versions.showMore': { other: '显示更多（还有 {count} 个）' },
  'mods.version.moreGameVersions': { other: '还有 {count} 个' },
  'mods.version.notFor': '不适用于 {loader} {gameVersion}',
  'mods.version.install': '安装',
  'mods.version.installed': '已安装 {title} {version}',
  'mods.version.installFailed': '无法安装 {title}：{error}',
  'mods.version.incompatibleTitle': '安装不兼容的版本？',
  'mods.version.installAnyway': '仍然安装',
  'mods.version.incompatibleBody':
    '{title} {version} 适用于 {loaders} {gameVersions}，而不是 {loader} {gameVersion}。它可能导致服务器崩溃。',
} satisfies CatalogOf<typeof en>;
