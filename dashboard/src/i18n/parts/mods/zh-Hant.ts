import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // utils/mods
  'mods.tab.mods': '模組',
  'mods.tab.plugins': '外掛',

  // ModsTab
  'mods.pageTitle.mod': '伺服器模組',
  'mods.pageTitle.plugin': '伺服器外掛',
  'mods.unsupported':
    '此伺服器不支援模組或外掛。Fabric、Forge、Paper 和 Spigot 伺服器支援。',
  'mods.noReadPermission': '你沒有讀取此實例檔案的權限。',
  'mods.view.installed': '已安裝',
  'mods.view.browse': '瀏覽',
  'mods.restartRequired.mod': '重新啟動伺服器以套用模組變更',
  'mods.restartRequired.plugin': '重新啟動伺服器以套用外掛變更',
  'mods.browse.placeholder.mod': '在 Modrinth 上搜尋模組，或貼上 Modrinth 連結',
  'mods.browse.placeholder.plugin':
    '在 Modrinth 上搜尋外掛，或貼上 Modrinth 連結',
  'mods.browse.onlyCompatible': '僅顯示適用於 {gameVersion} / {loader} 的版本',

  // common
  'mods.noWritePermission': '你沒有變更此伺服器檔案的權限',
  'mods.clientSide.playersNeedToo': '玩家也需要安裝',
  'mods.clientSide.serverOnly': '僅限伺服器',
  'mods.clientRequirement.required': '玩家需要安裝',
  'mods.clientRequirement.neededBy': '{mods} 需要它',
  'mods.clientRequirement.unknownModrinth':
    'Modrinth 未說明玩家是否需要它，請查看該模組的頁面',
  'mods.clientRequirement.unknownNotOnModrinth':
    '不在 Modrinth 上，請查看該模組的頁面',
  'mods.versionType.release': '正式版',
  'mods.versionType.beta': '測試版',
  'mods.versionType.alpha': '早期測試版',
  'mods.installResult.installed': '已安裝：{mods}',
  'mods.installResult.removed': '已移除：{files}',
  'mods.installResult.skipped': '已略過：{projects}',
  'mods.installResult.playersNeed': '玩家也需要安裝 {mods}',
  'mods.byAuthor': '作者：{author}',
  'mods.installedLabel': '已安裝',
  'mods.downloads': { other: '{count} 次下載' },
  'mods.followers': { other: '{count} 位追蹤者' },

  // ModrinthSearch
  'mods.sort.label': '排序方式',
  'mods.sort.relevance': '關聯性',
  'mods.sort.downloads': '下載次數',
  'mods.sort.follows': '追蹤數',
  'mods.sort.newest': '最新發布',
  'mods.sort.updated': '最近更新',
  'mods.search.ariaLabel': '搜尋 Modrinth',
  'mods.search.opening': '正在開啟連結…',
  'mods.search.linkFailed': '無法開啟連結：{error}',
  'mods.search.openLink': '開啟 {slug}',
  'mods.search.failed': '搜尋失敗：{error}',
  'mods.search.loading': '正在載入結果',
  'mods.search.noResults': '沒有結果。',
  'mods.search.page': '第 {page} 頁，共 {total} 頁',
  'mods.search.previousPage': '上一頁',
  'mods.search.nextPage': '下一頁',

  // InstalledMods
  'mods.installed.enableFailed': '無法啟用 {name}：{error}',
  'mods.installed.disableFailed': '無法停用 {name}：{error}',
  'mods.installed.updated': '已將 {name} 更新至 {version}',
  'mods.installed.updateFailed': '無法更新 {name}：{error}',
  'mods.installed.updateTo': '更新至 {version}',
  'mods.installed.viewOnModrinth': '在 Modrinth 上檢視',
  'mods.installed.deleteLabel': '刪除 {name}',
  'mods.installed.uploaded': '已上傳 {names}',
  'mods.installed.uploadFailed': '上傳失敗：{error}',
  'mods.installed.onlyJars': '只能上傳 .jar 檔案：{names}',
  'mods.installed.updatesInstalled': '已安裝更新',
  'mods.installed.someUpdatesFailed': '部分更新失敗：{failures}',
  'mods.installed.deleted': '已刪除 {name}',
  'mods.installed.deleteFailed': '刪除失敗：{error}',
  'mods.installed.deleteTitle.mod': '刪除模組？',
  'mods.installed.deleteTitle.plugin': '刪除外掛？',
  'mods.installed.deleteBody':
    '{name}（{fileName}）將從伺服器中刪除。此動作無法復原。',
  'mods.installed.overwriteTitle': '覆寫現有檔案？',
  'mods.installed.overwrite': '覆寫',
  'mods.installed.overwriteBody': '{message}要以上傳的檔案取代它嗎？',
  'mods.installed.overwriteDefault': '已安裝同名檔案。',
  'mods.installed.count.mod': { other: '{directory}/ 中有 {count} 個模組' },
  'mods.installed.count.plugin': { other: '{directory}/ 中有 {count} 個外掛' },
  'mods.installed.updateAll': '全部更新（{count}）',
  'mods.installed.checkUpdates': '檢查更新',
  'mods.installed.upload': '上傳',
  'mods.installed.loadFailed.mod': '無法載入模組：{error}',
  'mods.installed.loadFailed.plugin': '無法載入外掛：{error}',
  'mods.installed.loading': '正在載入模組',
  'mods.installed.empty.mod':
    '尚未安裝任何模組。瀏覽 Modrinth 或上傳 .jar 檔案。',
  'mods.installed.empty.plugin':
    '尚未安裝任何外掛。瀏覽 Modrinth 或上傳 .jar 檔案。',

  // ClientPackPanel
  'mods.clientPack.loader': 'Fabric Loader {version}',
  'mods.clientPack.loaderNoVersion': 'Fabric Loader',
  'mods.clientPack.plainFabric':
    '玩家只需原版 Fabric 即可加入（Minecraft {gameVersion}，{loader}）。',
  'mods.clientPack.title': { other: '玩家需要 {count} 個模組才能加入' },
  'mods.clientPack.download': '下載用戶端模組包（.mrpack）',
  'mods.clientPack.downloadFailed': '無法下載用戶端模組包：{error}',
  'mods.clientPack.importHint':
    '將其匯入 Prism Launcher、Modrinth App 或 ATLauncher，即可取得所有正確版本的內容。',
  'mods.clientPack.importHintWithJars': {
    other:
      '將其匯入 Prism Launcher、Modrinth App 或 ATLauncher，即可取得所有正確版本的內容。有 {count} 個模組不在 Modrinth 上，因此其檔案已包含在模組包中。',
  },

  // ModrinthProjectView
  'mods.project.loading': '正在載入專案',
  'mods.project.loadFailed': '無法載入專案：{error}',
  'mods.project.link.source': '原始碼',
  'mods.project.link.issues': '問題回報',
  'mods.project.link.wiki': 'Wiki',
  'mods.project.serverSide': '伺服器：{value}',
  'mods.project.side.required': '必要',
  'mods.project.side.optional': '選用',
  'mods.project.side.unsupported': '不支援',
  'mods.project.side.unknown': '未知',
  'mods.project.createFromModpack': '用此模組包建立伺服器',
  'mods.project.modpackNoPermission':
    '這是一個模組包。你沒有建立伺服器的權限。',
  'mods.project.noServerSupport': '此專案表示它無法在伺服器上執行。',
  'mods.project.wrongType.mod':
    '這是一個 {type} 專案。Lodestone 在這裡只安裝模組。',
  'mods.project.wrongType.plugin':
    '這是一個 {type} 專案。Lodestone 在這裡只安裝外掛。',
  'mods.project.description': '說明',
  'mods.versions.title': '版本',
  'mods.versions.withDependencies': '同時安裝必要的相依項目',
  'mods.versions.showAll': '顯示所有版本',
  'mods.versions.loadFailed': '無法載入版本：{error}',
  'mods.versions.loading': '正在載入版本',
  'mods.versions.none': '沒有適用於 {loader} {gameVersion} 的版本。',
  'mods.versions.noneShowAll':
    '沒有適用於 {loader} {gameVersion} 的版本。開啟「顯示所有版本」以查看其他版本。',
  'mods.versions.showMore': { other: '顯示更多（還有 {count} 個）' },
  'mods.version.moreGameVersions': { other: '還有 {count} 個' },
  'mods.version.notFor': '不適用於 {loader} {gameVersion}',
  'mods.version.install': '安裝',
  'mods.version.installed': '已安裝 {title} {version}',
  'mods.version.installFailed': '無法安裝 {title}：{error}',
  'mods.version.incompatibleTitle': '安裝不相容的版本？',
  'mods.version.installAnyway': '仍要安裝',
  'mods.version.incompatibleBody':
    '{title} {version} 適用於 {loaders} {gameVersions}，而非 {loader} {gameVersion}。它可能導致伺服器當機。',
} satisfies CatalogOf<typeof en>;
