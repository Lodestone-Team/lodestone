import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // InstanceCreateForm / ModpackCreateForm: the steps in the dialog's sidebar
  'create.step.selectGame': '選擇遊戲',
  'create.step.basicSettings': '基本設定',
  'create.step.serverSettings': '伺服器設定',
  'create.step.autoSettings': '自動化設定',
  'create.step.selectModpack': '選擇模組包',
  'create.step.selectVersion': '選擇版本',

  // InstanceCreateForm: the pages' subtitles, buttons and errors
  'create.basicSettings.description': '伺服器的基本設定。',
  'create.serverSettings.description': '設定你的伺服器。',
  'create.autoSettings.description': '自動設定你的伺服器。',
  'create.checkFields': '請確認所有欄位都已正確填寫',
  'create.createServer': '建立伺服器',
  'create.createFailed': '建立伺服器時發生錯誤：{error}',
  'create.unknownValueType': '產生表單時發生錯誤：未知的值類型。',

  // form.ts and ModpackCreateForm: the dashboard's own setup fields
  'create.field.name': '名稱',
  'create.field.nameDescription': '伺服器的名稱',
  'create.field.nameDefault': '我的伺服器',
  'create.field.description': '說明',
  'create.field.descriptionDescription': '伺服器的說明',
  'create.field.port': '連接埠',
  'create.field.portDescription': '伺服器執行時使用的連接埠',
  'create.field.minRam': '最小記憶體',
  'create.field.minRamDescription': '分配給伺服器的最小記憶體，單位為 MB',
  'create.field.maxRam': '最大記憶體',
  'create.field.maxRamDescription': '分配給伺服器的最大記憶體，單位為 MB',
  'create.field.autoStart': '自動啟動',
  'create.field.autoStartDescription': '應用程式啟動時自動啟動伺服器',
  'create.field.restartOnCrash': '當機時重新啟動',
  'create.field.restartOnCrashDescription': '伺服器當機時自動重新啟動',

  // form.ts: validation of a setting; {name} is the setting's name (often sent by the core, untranslated)
  'create.validation.required': '「{name}」為必填欄位',
  'create.validation.number': '「{name}」必須是數字',
  'create.validation.integer': '「{name}」必須是整數',
  'create.validation.positive': '「{name}」必須為正數',
  'create.validation.min': '「{name}」必須大於或等於 {min}',
  'create.validation.max': '「{name}」必須小於或等於 {max}',
  'create.validation.oneOf': '「{name}」必須是可用的選項之一',

  // FormComboField: the options list of a field
  'create.combo.add': '新增「{query}」',
  'create.combo.nothingFound': '找不到任何結果',

  // GameTypeSelectForm: the first step, picking a game
  'create.gameType.title': '選擇遊戲',
  'create.gameType.subtitle': '你的伺服器要用來做什麼？',
  'create.gameType.docker': '了解 Docker 伺服器（測試版）',
  'create.gameType.new': '新功能',
  'create.gameType.fetchFailed': '取得伺服器時發生錯誤',
  'create.gameType.modpack': 'Modrinth 模組包',
  'create.gameType.modpackDescription': '從 Modrinth 模組包建立 Fabric 伺服器',

  // GameTypeMappings: games' titles and descriptions
  'create.game.genericTitle': '{name}（通用）',
  'create.game.vanillaDescription': 'Mojang 官方的標準原版 Minecraft 伺服器。',
  'create.game.forgeDescription':
    '模組框架，可用來安裝模組並自訂你的 Minecraft 體驗。',
  'create.game.fabricDescription': '輕量級的 Minecraft 模組工具鏈。',
  'create.game.paperDescription':
    '高效能的 Spigot 分支，旨在修正遊戲玩法與機制上的不一致。',
  'create.game.spigotDescription':
    '經過修改的 Minecraft 伺服器軟體，支援外掛程式，提供更佳的效能與更多自訂選項。',
  'create.game.otherDescription': '未知的 Minecraft 變體：{name}',
  'create.game.bedrockDescription': 'Minecraft Bedrock Edition 伺服器。',
  'create.game.genericDescription': '未知的遊戲：{name}',

  // CreateInstanceFlow: read by screen readers while the dialog loads
  'create.loadingForm': '正在載入建立表單',

  // ModpackCreateForm: creating a server from a Modrinth modpack
  'create.modpack.sourceTitle': '選擇模組包',
  'create.modpack.sourceDescription':
    '搜尋 Modrinth、貼上 Modrinth 連結，或上傳 .mrpack 檔案。僅支援 Fabric 模組包。',
  'create.modpack.searchPlaceholder': '搜尋模組包，或貼上 Modrinth 連結',
  'create.modpack.onlyFabric': '僅顯示 Fabric 模組包',
  'create.modpack.upload': '上傳 .mrpack',
  'create.modpack.versionTitle': '選擇版本',
  'create.modpack.versionUploaded': '已上傳的模組包。',
  'create.modpack.versionDescription': '選擇要安裝的模組包版本。',
  'create.modpack.downloading': '正在下載模組包檔案以讀取其內容…',
  'create.modpack.downloadingSize': '正在下載模組包檔案（{size}）以讀取其內容…',
  'create.modpack.versionHint':
    '選擇一個版本以查看其內容。Lodestone 會下載該版本的模組包檔案來讀取；在你點擊「建立伺服器」之前，不會安裝任何內容。',
  'create.modpack.settingsTitle': '伺服器設定',
  'create.modpack.loadingModpack': '正在載入模組包',
  'create.modpack.loadingVersions': '正在載入版本',
  'create.modpack.versionsFailed': '無法載入版本：{error}',
  'create.modpack.noVersions': '此模組包沒有任何版本。',
  'create.modpack.select': '選擇',
  'create.modpack.selected': '已選擇',
  'create.modpack.showMore': {
    other: '顯示更多（還有 {count} 個）',
  },
  // the summary of the picked version
  'create.modpack.summary.modpack': '模組包',
  'create.modpack.summary.loader': '載入器',
  'create.modpack.summary.serverFiles': '伺服器檔案',
  'create.modpack.summary.serverFilesValue': '{count} 個（需下載 {size}）',
  'create.modpack.unsupported': '無法使用此模組包：{reason}',
  // toasts and errors
  'create.modpack.inspectFailed': '無法讀取模組包：{error}',
  'create.modpack.notAModpack': '這是 {type}，不是模組包',
  'create.modpack.chooseMrpack': '請選擇 .mrpack 檔案',
  'create.modpack.noUploadId': '核心未傳回上傳 ID',
  'create.modpack.uploadFailed': '無法上傳模組包：{error}',
  // validation of the settings step
  'create.modpack.nameRequired': '名稱為必填欄位',
  'create.modpack.portNumber': '連接埠必須是數字',
  'create.modpack.portInteger': '連接埠必須是整數',
  'create.modpack.portRange': '連接埠必須介於 1 到 65535 之間',
  'create.modpack.portRequired': '連接埠為必填欄位',
  'create.modpack.minRamNumber': '最小記憶體必須是數字',
  'create.modpack.minRamInteger': '最小記憶體必須是整數',
  'create.modpack.minRamMin': '最小記憶體至少為 1 MB',
  'create.modpack.minRamRequired': '最小記憶體為必填欄位',
  'create.modpack.maxRamNumber': '最大記憶體必須是數字',
  'create.modpack.maxRamInteger': '最大記憶體必須是整數',
  'create.modpack.maxRamMin': '最大記憶體不能小於最小記憶體',
  'create.modpack.maxRamRequired': '最大記憶體為必填欄位',
} satisfies CatalogOf<typeof en>;
