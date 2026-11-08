import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // tabList
  'server.tab.overview': '概覽',
  'server.tab.settings': '設定',
  'server.tab.console': '主控台',
  'server.tab.files': '檔案',
  'server.tab.mods': '模組',
  'server.tab.macros': '巨集',
  'server.tab.logs': '事件日誌',
  'server.tab.playitgg': 'Playitgg',

  // InstanceTabs
  'server.notFound': '找不到伺服器',
  'server.tabNotFound': '找不到 {path}',
  'server.logs.comingSoon': '即將推出，敬請期待！',

  // InstanceOverview
  'server.overview.pageTitle': '伺服器概覽',
  'server.overview.noName': '無名稱',
  'server.overview.noDescription': '無說明',
  'server.overview.version': '版本 {version}',
  'server.overview.playerCount': '玩家數 {players}/{max}',

  // InstanceDetails
  'server.details.title': '詳細資訊',
  'server.details.name': '名稱',
  'server.details.description': '說明',
  'server.details.uuid': 'UUID',
  'server.details.game': '遊戲',
  'server.details.minecraftVersion': 'Minecraft 版本',
  'server.details.version': '版本',
  'server.details.loader': '載入器',
  'server.details.port': '連接埠',
  'server.details.state': '狀態',
  'server.details.players': '玩家',
  'server.details.created': '建立時間',
  'server.details.autoStart': '自動啟動',
  'server.details.restartOnCrash': '當機後重新啟動',
  'server.details.ram': '最小 / 最大記憶體',
  'server.details.ramValue': '{min} MB / {max} MB',
  'server.details.folder': '資料夾',
  'server.details.modpack': '建立自模組包',

  // InstancePerformance
  'server.performance.title': '伺服器監控',
  'server.performance.lead': '此實例對你的 Lodestone Core 的資源使用量',
  'server.performance.cpuLoad': 'CPU 負載：{value}%',
  'server.performance.memoryUsed': '已用記憶體：{value} GiB',
  'server.performance.loadingGraph': '正在載入圖表',
  'server.performance.cpuUsage': 'CPU 使用率',
  'server.performance.memoryUsage': '記憶體使用量',
  'server.performance.unitPercent': '%',
  'server.performance.unitGiB': ' GiB',

  // PerformanceGraph
  'server.graph.seconds': '{seconds} 秒',
  'server.graph.tooltipTime': '{time}：',

  // InstancePlayerList
  'server.players.title': '玩家清單',
  'server.players.lead': '目前在線上的所有玩家',
  'server.players.sortByName': '名稱',
  'server.players.avatarAlt': '{name} 的頭像',
  'server.players.linkedUser': '連結此 Minecraft 帳戶的 Lodestone 使用者',
  'server.players.none': '沒有在線上的玩家',

  // InstanceSettings
  'server.settings.pageTitle': '伺服器設定',
  'server.settings.automation': '自動化',
  'server.settings.automationLead': 'Lodestone 何時自動啟動此伺服器。',
  'server.settings.autoStart': '自動啟動',
  'server.settings.autoStartHint': 'Lodestone Core 啟動時啟動此伺服器',
  'server.settings.restartOnCrash': '當機後重新啟動',
  'server.settings.restartOnCrashHint': '此伺服器當機時重新啟動它',
  'server.settings.notAvailable': '暫時無法使用',
  'server.settings.notAvailableHint': '請至少啟動此伺服器一次',
  'server.settings.noPermission': '沒有權限',
  'server.settings.dangerZone': '危險區域',
  'server.settings.dangerZoneLead':
    '這些設定可能對你的伺服器造成無法復原的損害。',
  'server.settings.deleteServer': '刪除伺服器',
  'server.settings.deleteServerHint': '永久刪除此伺服器及其資料',
  'server.settings.deleteTitle': '刪除「{name}」',
  'server.settings.deleteConfirm': '我了解，刪除此伺服器',
  'server.settings.deleteCannotUndo': '此動作無法復原。',
  'server.settings.deleteWarning':
    '{cannotUndo}此實例的設定、世界和備份將被永久刪除。繼續之前，請備份所有重要資料。',

  // SettingField
  'server.settingField.unknownType': '提交錯誤：未知的值類型。',

  // PublicListingSetting
  'server.publicListing.title': '分享',
  'server.publicListing.lead': '誰可以在不登入的情況下找到此伺服器。',
  'server.publicListing.listedPublicly': '公開列出',
  'server.publicListing.public': '公開',
  'server.publicListing.ownerOnly': '只有擁有者可以變更此項',
  'server.publicListing.flag.status': '狀態和玩家',
  'server.publicListing.flag.address': '加入位址',
  'server.publicListing.flag.clientPack': '用戶端模組下載',
  'server.publicListing.shownText':
    '任何能存取此核心的人無需登入即可看到此伺服器的{list}。',
  'server.publicListing.shown.nameVersion': '名稱和版本',
  'server.publicListing.shown.status': '狀態和玩家數',
  'server.publicListing.shown.address': '位址',
  'server.publicListing.shown.clientPack': '用戶端模組',
  'server.publicListing.turnOnFirst': '請先開啟「公開列出伺服器」。',
  'server.publicListing.hiddenWhileOff':
    '{shown}「公開列出伺服器」關閉時會隱藏。',
  'server.publicListing.notShown': '未登入時不會顯示。',
  'server.publicListing.noServers': '還沒有 Minecraft Java Edition 伺服器。',

  // ResourcePackSetting
  'server.resourcePack.title': '資源包',
  'server.resourcePack.lead': '玩家加入時會被要求下載的資源包。',
  'server.resourcePack.url': 'URL',
  'server.resourcePack.urlHint': '資源包 zip 的直接連結。留空則不使用資源包。',
  'server.resourcePack.sha1Hint': '讓玩家可以驗證下載內容並快取資源包。',
  'server.resourcePack.sha1Error': 'SHA-1 應為 40 個十六進位字元。',
  'server.resourcePack.compute': '計算',
  'server.resourcePack.downloaded': '已下載 {size}。',
  'server.resourcePack.required': '強制使用資源包',
  'server.resourcePack.requiredHint': '拒絕資源包的玩家會被中斷連線。',
  'server.resourcePack.prompt': '提示訊息',
  'server.resourcePack.promptHint':
    '詢問玩家時顯示的訊息。純文字或 JSON 文字元件。',
  'server.resourcePack.remove': '移除資源包',
  'server.resourcePack.saved': '資源包已儲存',
  'server.resourcePack.savedRestart': '資源包已儲存。重新啟動伺服器後生效。',

  // InstanceCard
  'server.action.start': '啟動',
  'server.action.stop': '停止',
  'server.action.restart': '重新啟動',
  'server.action.kill': '強制停止',
  'server.card.power': '電源',
  'server.card.noScripting':
    '此核心建置時未包含指令碼支援（Deno），因此無法使用 atom 伺服器。',
  'server.card.codeExecutionDisabled': '此核心已停用程式碼執行',
  'server.card.copyIp': '複製 IP',
  'server.card.online': '{players}/{max} 在線上',

  // InstanceLoadingPill
  'server.loading.settingUp': '正在設定…（{percent}）',

  // apis.ts
  'server.api.deletedFile': '已刪除檔案：{name}',
  'server.api.deletedDirectory': '已刪除資料夾：{path}',
  'server.api.downloadFailed': '下載 {name} 失敗：{error}',
  'server.api.uploading': {
    other: '正在上傳 {count} 個檔案',
  },
  'server.api.moveFailed': '移動 {name} 失敗：{error}',
  'server.api.zipFailed': '無法開始壓縮：{error}',
  'server.api.zipping': {
    other: '正在壓縮 {count} 個項目…',
  },
  'server.api.unzipFailed': '解壓縮 {name} 失敗：{error}',
  'server.api.unzipping': '正在解壓縮 {name}…',
  'server.api.tooManyAttemptsSeconds': {
    other: '登入失敗次數過多。請等待 {count} 秒後再試一次。',
  },
  'server.api.tooManyAttemptsMinutes': {
    other: '登入失敗次數過多。請等待 {count} 分鐘後再試一次。',
  },
  'server.api.tooManyAttempts': '登入失敗次數過多。請等待幾分鐘後再試一次。',
  'server.api.loginFailed': '登入失敗：{message}',
  'server.api.loginError': '錯誤：{message}',
} satisfies CatalogOf<typeof en>;
