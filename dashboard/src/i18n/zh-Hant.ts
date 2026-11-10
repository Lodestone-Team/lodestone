// 繁體中文目錄。鍵和預留位置與 en.ts 相同（由 tsc 和 catalogs.test.ts 檢查）。
//
// 風格：臺灣常用介面用語（檔案、資料夾、設定、網路、使用者、預設），稱呼用「你」；
// 全形標點；中文與拉丁字母、數字之間加半形空格（如「啟動 Minecraft 伺服器」）；
// 按鈕和標籤不加句號。產品名（Lodestone、Lodestone Core、Minecraft、Modrinth）
// 不翻譯。「伺服器」只指遊戲伺服器；Lodestone Core 稱為「核心」。
//
// 術語表：
//   server (a game server)           伺服器
//   core (Lodestone Core)            核心
//   instance                         實例
//   dashboard                        控制面板
//   console                          主控台
//   settings                         設定
//   user / owner                     使用者 / 擁有者
//   permission                       權限
//   sign in / sign out               登入 / 登出
//   password / username              密碼 / 使用者名稱
//   API key / token                  API 金鑰 / 權杖
//   file / folder                    檔案 / 資料夾
//   upload / download                上傳 / 下載
//   mod / modpack                    模組 / 模組包
//   resource pack                    資源包
//   macro                            巨集
//   player / world                   玩家 / 世界
//   port / address                   連接埠 / 位址
//   memory (RAM)                     記憶體
//   start / stop / restart / kill    啟動 / 停止 / 重新啟動 / 強制停止
//   running / stopped                執行中 / 已停止
//   log / event                      日誌 / 事件
//   save / delete / remove           儲存 / 刪除 / 移除
//   enable / disable                 啟用 / 停用
//   connection / network             連線 / 網路
//   default / search                 預設 / 搜尋
//   public listing                   公開列表
import type { Messages } from './types.ts';

export const zhHant: Messages = {
  // ===== Common words =====

  'common.cancel': '取消',
  'common.confirm': '確認',
  'common.save': '儲存',
  'common.saved': '已儲存',
  'common.saving': '正在儲存…',
  'common.delete': '刪除',
  'common.remove': '移除',
  'common.rename': '重新命名',
  'common.edit': '編輯',
  'common.create': '建立',
  'common.add': '新增',
  'common.close': '關閉',
  'common.back': '返回',
  'common.next': '下一步',
  'common.done': '完成',
  'common.copy': '複製',
  'common.copied': '已複製',
  'common.copiedToClipboard': '已複製到剪貼簿',
  'common.retry': '重試',
  'common.refresh': '重新整理',
  'common.search': '搜尋',
  'common.loading': '載入中…',
  'common.yes': '是',
  'common.no': '否',
  'common.ok': '確定',
  'common.on': '開',
  'common.off': '關',
  'common.enabled': '已啟用',
  'common.disabled': '已停用',
  'common.none': '無',
  'common.unknown': '不明',
  'common.default': '預設',
  'common.optional': '選填',
  'common.required': '必填',
  'common.learnMore': '瞭解詳情',
  'common.showMore': '顯示更多',
  'common.showLess': '收合',
  'common.you': '你',
  'common.unknownError': '不明錯誤',
  'common.networkError': '網路錯誤',
  'common.networkErrorWithMessage': '網路錯誤：{message}',
  'common.invalidToken': '權杖無效：請登出後重新登入',
  'common.notAnInteger': '不是整數',
  'common.notANumber': '不是有效的數字',
  'common.justNow': '剛剛',
  'common.pageTitle': '{page} - Lodestone',
  'common.state.Starting': '啟動中',
  'common.state.Running': '執行中',
  'common.state.Stopping': '停止中',
  'common.state.Stopped': '已停止',
  'common.state.Error': '錯誤',
  // Form fields and settings rows (Atoms/Form, Atoms/Config)
  'common.fieldLabel': '{label}：',
  'common.select': '請選擇…',
  'common.noPermission': '無權限',
  'common.input.cannotBeEmpty': '不可空白',
  'common.input.mustBeNumber': '必須是數字',
  'common.input.mustBeGreaterThan': '必須大於 {min}',
  'common.input.mustBeLessThan': '必須小於 {max}',
  'common.combo.addCustom': '新增「{query}」',
  'common.combo.nothingFound': '找不到結果。',
  // Table
  'common.table.noData': '目前沒有資料',
  // ClipboardTextfield: text that copies itself when clicked
  'common.clipboard.copyToClipboard': '複製到剪貼簿',
  'common.clipboard.copiedText': '已複製「{text}」',
  // The server resource pack URL check (utils/resourcePack.ts)
  'common.resourcePack.urlSpaces': 'URL 不可包含空格。',
  'common.resourcePack.notAUrl': '這不是有效的 URL。',
  'common.resourcePack.urlScheme': 'URL 必須以 http:// 或 https:// 開頭。',

  // ===== App shell: navigation, top bar, settings frame, sign-in and setup, notifications =====

  'shell.session.expired': '你的工作階段已過期，請重新登入',
  'shell.session.invalid': '你的工作階段已失效，請重新登入',
  'shell.token.invalid': '權杖無效',
  'shell.token.invalidExp': '權杖中的 exp 無效',
  'shell.token.invalidUid': '權杖中的 uid 無效',
  'shell.token.expired': '權杖已過期',

  'shell.error.invalidStatusCode': '狀態碼無效',
  'shell.error.invalidResponse': '回應無效',

  'shell.settings.general': '一般',
  'shell.settings.users': '使用者',
  'shell.settings.profile': '個人資料',

  'shell.settings.pagesButton': '設定頁面',
  'shell.settings.close': '關閉設定',
  'shell.settings.closeTooltip': '關閉（Esc）',

  'shell.settingsNav.label': '設定',
  'shell.settingsNav.core': '核心',
  'shell.settingsNav.account': '帳戶',
  'shell.settingsNav.users': '使用者',
  'shell.settingsNav.unselectUser': '取消選取使用者',

  'shell.beta.title': '感謝使用 Lodestone 測試版！',
  'shell.beta.body':
    '感謝使用 Lodestone 測試版！如果你發現任何問題，請在我們的 Discord 伺服器或 Github 上告訴我們。',
  'shell.versionMismatch.title': '需要更新！',
  'shell.versionMismatch.continue': '我已瞭解，不更新並繼續',
  'shell.versionMismatch.coreVersion': '核心版本：',
  'shell.versionMismatch.dashboardVersion': '控制面板版本：',
  'shell.versionMismatch.incompatible': '你的控制面板與核心不相容！',
  'shell.versionMismatch.body':
    '這可能導致非預期的行為。請將核心更新至最新版本。詳情請參閱 {link}。',
  'shell.versionMismatch.wiki': 'Wiki',
  'shell.versionMismatch.toast':
    '版本不一致！你的核心是否已過時？核心：{core}，控制面板：{dashboard}',
  'shell.localCore.title': '偵測到新的本機核心',
  'shell.localCore.body': '偵測到一個尚未設定的本機核心。要設定 {name} 嗎？',
  'shell.localCore.setup': '設定',
  'shell.localCore.skip': '略過',
  'shell.setupRequired.title': '需要設定',
  'shell.setupRequired.body': '{name} 尚未設定。請完成設定流程。',
  'shell.setupRequired.changeCore': '更換核心',
  'shell.coreError.title': '核心連線錯誤',
  'shell.coreError.body':
    '連線至 {address} 時發生錯誤。請選擇其他核心、重新整理頁面，或等待核心重新上線。',
  'shell.coreError.back': '返回核心選擇',
  'shell.coreError.continue': '繼續使用目前的核心',

  'shell.topNav.degraded': '{refresh}以在 Firefox 上取得最新資料。',
  'shell.topNav.refresh': '重新整理',
  'shell.topNav.devBuild': '本機開發版本',

  'shell.connection.connected': '已連線',
  'shell.connection.connectedDetail': '正在接收即時更新',
  'shell.connection.connecting': '連線中…',
  'shell.connection.connectingDetail': '正在等待核心回應',
  'shell.connection.degraded': '連線受限',
  'shell.connection.degradedDetail': '無法使用即時更新，請重新整理以更新',
  'shell.connection.disconnected': '已中斷連線',
  'shell.connection.disconnectedDetail': '無法連線至核心',
  'shell.connection.address': '位址',
  'shell.connection.version': '版本',
  'shell.connection.host': '主機',
  'shell.connection.system': '系統',
  'shell.connection.uptime': '執行時間',
  'shell.connection.uptimeDays': '{days} 天 {hours} 小時',
  'shell.connection.uptimeHours': '{hours} 小時 {minutes} 分鐘',
  'shell.connection.uptimeMinutes': '{minutes} 分鐘',
  'shell.connection.coreFallback': '核心',
  'shell.connection.ariaLabel': '{core}：{status}',

  'shell.nav.allInstances': '所有實例',
  'shell.nav.integrations': '整合',
  'shell.nav.selectedInstance': '已選取的實例',
  'shell.nav.noSelection': '選取的伺服器會顯示在這裡',
  'shell.nav.loadingServers': '正在載入伺服器',
  'shell.nav.logInToView': '登入以檢視你的伺服器。',
  'shell.nav.newServer': '新增伺服器…',
  'shell.nav.noPermission': '沒有權限',

  'shell.notifications.title': '通知',
  'shell.notifications.inProgress': '進行中',
  'shell.notifications.all': '所有通知',
  'shell.notifications.loading': '正在載入通知',
  'shell.notifications.empty': '目前沒有通知！',

  'shell.event.state.Starting': '伺服器 {name} 正在啟動！',
  'shell.event.state.Running': '伺服器 {name} 已啟動！',
  'shell.event.state.Stopping': '伺服器 {name} 正在停止！',
  'shell.event.state.Stopped': '伺服器 {name} 已停止！',
  'shell.event.state.Error': '伺服器 {name} 發生錯誤！',
  'shell.event.warningToast':
    '警告：某個伺服器發出了警告。這不應該發生，請向開發者回報此問題。',
  'shell.event.warning': '伺服器 {name} 發出了警告',
  'shell.event.error': '伺服器 {name} 發生錯誤',
  'shell.event.playersJoined': '{players} 加入了 {name}',
  'shell.event.playersLeft': '{players} 離開了 {name}',
  'shell.event.playersJoinedAndLeft':
    '{joined} 加入了 {name}，{left} 離開了 {name}',
  'shell.event.playerMessage': '{player} 在 {name} 上說：{message}',
  'shell.event.apiKeyCreated': '已建立 API 金鑰「{name}」',
  'shell.event.apiKeyRevoked': '已撤銷 API 金鑰「{name}」',
  'shell.event.macroStarted': '巨集 {pid} 已在 {server} 上啟動',
  'shell.event.macroDetached': '巨集 {pid} 已在 {server} 上分離',
  'shell.event.macroStopped':
    '巨集 {pid} 已在 {server} 上停止，狀態為 {status}',
  'shell.event.loadFailed': '無法載入通知：{error}',

  'shell.noScripting':
    '此核心建置時未包含指令碼支援（Deno），因此無法使用巨集。',

  'shell.userMenu.guest': '訪客',
  'shell.userMenu.account': '帳戶：{name}',
  'shell.userMenu.signedInAs': '已登入為 {name}',
  'shell.userMenu.notSignedIn': '未登入',
  'shell.userMenu.signOut': '登出',
  'shell.userMenu.signIn': '登入',
  'shell.userMenu.changeCore': '更換核心',
  'shell.userMenu.settings': '設定',

  'shell.userBox.changePassword': '變更密碼',
  'shell.userBox.cantDeleteSelfTitle': '你無法刪除自己',
  'shell.userBox.cantDeleteSelf': '你無法刪除自己。',
  'shell.userBox.deleteTitle': '刪除使用者',
  'shell.userBox.deleteConfirm': '確定要刪除 {name} 嗎？此動作無法復原。',
  'shell.userBox.deleteUser': '刪除使用者',
  'shell.userBox.admin': '管理員',
  'shell.userBox.owner': '擁有者',

  'shell.connectionHelp.trustCertificate':
    '如果核心正在執行並使用內建憑證，此瀏覽器需要信任該憑證一次。開啟 {link}，接受憑證警告，然後返回重試。',
  'shell.connectionHelp.fingerprint':
    '為確認這是你的核心，請將瀏覽器顯示的憑證指紋與核心日誌或核心設定中的指紋進行比對。',
  'shell.connectionHelp.plainHttp':
    '根據預設，Lodestone 核心會拒絕來自其他裝置的純 HTTP 連線。',
  'shell.connectionHelp.savedCore':
    '請使用相同位址和 https 通訊協定連線至新的核心。',
  'shell.connectionHelp.changeProtocol': '請將通訊協定改為 https 後重試。',

  'shell.login.checkFields': '{error}：請確認各欄位填寫正確。',
  'shell.login.invalidResponse': '回應無效，設定可能有誤',
  'shell.login.downloadCore': '下載 Lodestone Core',
  'shell.login.continue': '繼續',
  'shell.login.or': '或',
  'shell.login.username': '使用者名稱',
  'shell.login.password': '密碼',
  'shell.login.usernameRequired': '請輸入使用者名稱',
  'shell.login.passwordRequired': '請輸入密碼',
  'shell.login.changeCore': '更換核心',
  'shell.login.signIn': '登入',
  'common.localOnly':
    '這個核心只允許它所在電腦上的應用程式使用。請在那台電腦上開啟 Lodestone Host 來管理它，或在那裡設定擁有者帳戶（設定 › 一般 › 允許其他裝置存取）以使用網頁控制台。',
  'shell.localOnly.title': '這個核心只允許它所在的電腦使用',
  'shell.login.localOnly':
    '這個核心只允許這台電腦上的應用程式使用。請開啟 Lodestone Host 來管理它，或在那裡設定擁有者密碼（設定 › 一般 › 允許其他裝置存取）以使用網頁控制台。',

  'shell.login.withApiKey': '使用 API 金鑰登入',
  'shell.login.withPassword': '使用使用者名稱和密碼登入',
  'shell.login.apiKey': 'API 金鑰',
  'shell.login.apiKeyHint':
    '貼上以 lsk_ 開頭的金鑰。你會取得該金鑰的權限，撤銷金鑰後會登出。',
  'shell.login.apiKeyRequired': '需要 API 金鑰',
  'shell.login.apiKeyFormat': 'API 金鑰以 lsk_ 開頭',
  'shell.coreConnect.pageTitle': '連線至核心',
  'shell.coreConnect.title': '新增核心',
  'shell.coreConnect.httpErrorTitle': 'HTTP 錯誤',
  'shell.coreConnect.mixedContent':
    '缺少網站權限。請允許混合內容，或停用所有 HTTPS 瀏覽器外掛程式，然後重新整理頁面。{link}',
  'shell.coreConnect.httpsWarning': '你可能需要調整網路和瀏覽器設定。{link}',
  'shell.coreConnect.apiVersion': 'API 版本',
  'shell.coreConnect.protocol': '通訊協定',
  'shell.coreConnect.port': '連接埠',
  'shell.coreConnect.address': 'IP 位址/網域',
  'shell.coreConnect.addressPlaceholder':
    '例如 123.123.123.123 或「myserver.com」',
  'shell.coreConnect.submit': '新增並繼續',

  'shell.coreSelect.pageTitle': '選擇核心',
  'shell.coreSelect.title': '選擇 Lodestone Core',
  'shell.coreSelect.existing': '選擇現有核心',
  'shell.coreSelect.connectNew': '連線至新的核心',

  'shell.coreSetup.pageTitle': '設定新核心',
  'shell.coreSetup.title': '建立擁有者帳戶',
  'shell.coreSetup.findKey':
    '請在核心的主控台輸出中尋找「First time setup key」',
  'shell.coreSetup.setupKey': '設定金鑰',
  'shell.coreSetup.setupKeyPrefilled': '設定金鑰（已預先填入）',
  'shell.coreSetup.confirmPassword': '確認密碼',
  'shell.coreSetup.submit': '提交',
  'shell.coreSetup.passwordConfirmRequired': '請確認密碼',
  'shell.coreSetup.passwordMismatch': '兩次輸入的密碼不一致',
  'shell.coreSetup.setupKeyRequired': '請輸入設定金鑰',
  'shell.coreSetup.failed': '設定核心時發生錯誤',

  'shell.coreConfig.pageTitle': '設定新核心',
  'shell.coreConfig.title': '自訂你的核心',
  'shell.coreConfig.coreName': '核心名稱',
  'shell.coreConfig.domain': '公開網域/IP（選填）',
  'shell.coreConfig.coreNameRequired': '請輸入核心名稱',
  'shell.coreConfig.domainTooLong': '網域不得超過 {max} 個字元',

  'shell.welcome.pageTitle': '歡迎使用 Lodestone',
  'shell.welcome.wiki': 'Wiki 與常見問題',
  'shell.welcome.title': '歡迎使用 Lodestone！',
  'shell.welcome.version': '版本 {version}',
  'shell.welcome.beta': 'Lodestone 是測試版軟體！',
  'shell.welcome.betaBody':
    '瀏覽器支援有限，可能會遇到錯誤。不保證功能的穩定性與未來的相容性。你可以在我們的 {github} 上查看已知問題並回報新問題。',
  'shell.welcome.connectExisting': '連線至現有核心',
  'shell.welcome.setupCore': '設定 Lodestone Core',

  'shell.userLogin.switchAccount': '切換帳戶',
  'shell.userSelect.pageTitle': '選擇使用者',
  'shell.userSelect.loadingUser': '正在載入使用者',
  'shell.userSelect.continueAs': '以 {name} 繼續',
  'shell.userSelect.continueAsCurrent': '以目前的使用者繼續',
  'shell.userSelect.switchUser': '切換使用者',

  'shell.home.pageTitle': '首頁',
  'shell.home.title': '管理你的伺服器',
  'shell.home.subtitle': '首先，從選單中選擇一個伺服器，或新增一個',
  'shell.home.create': '新增伺服器',

  'shell.notFound.title': '找不到頁面',
  'shell.notFound.goHome': '返回首頁',

  // ===== Settings: core settings, users, profile, API keys =====

  'settings.language.label': '語言',
  'settings.language.auto': '跟隨瀏覽器（{language}）',

  'settings.theme.label': '主題',
  'settings.theme.system': '跟隨系統',
  'settings.theme.light': '淺色',
  'settings.theme.dark': '深色',
  'settings.theme.systemHint': '跟隨裝置的淺色或深色設定',
  'settings.theme.lightHint': '一律使用淺色',
  'settings.theme.darkHint': '一律使用深色',

  'settings.toc.label': '章節',

  'settings.legacy.title': '核心設定',

  'settings.core.documentTitle': 'Lodestone Core 設定',
  'settings.core.title': '一般',
  'settings.core.section.core': '核心',
  'settings.core.section.public': '公開伺服器',
  'settings.core.section.publicLead': '無須登入即可查看。',
  'settings.core.section.info': '資訊',
  'settings.core.section.danger': '危險區域',
  'settings.core.section.dangerLead': '請謹慎變更。',
  'settings.core.name.label': '核心名稱',
  'settings.core.name.empty': '名稱不能是空白',
  'settings.core.name.tooLong': '名稱不能超過 {max} 個字元',
  'settings.core.domain.label': '公開網域或 IP',
  'settings.core.domain.tooLong': '網域不能超過 {max} 個字元',
  'settings.core.domain.tooltip':
    '如果你已設定連接埠轉送，這裡填入你的公開 IP。否則請維持不變。',
  'settings.core.listPublicly.label': '公開列出伺服器',
  'settings.core.listPublicly.hint':
    '任何能連到此核心的人都能看到你在下方設為公開的伺服器。',
  'settings.core.localClaim.label': '允許本機上的應用程式取得擁有者權限',
  'settings.core.localClaim.hint':
    'Lodestone Launcher 可以藉由證明自己能寫入此核心的資料夾來成為擁有者。能寫入該資料夾的程式本來就能修改所有使用者和設定，所以這只在共用電腦上有影響。',
  'settings.core.safeMode.label': '安全模式',
  'settings.core.safeMode.hint': '僅擁有者可取得此電腦的管理權限。',
  'settings.core.safeMode.offTitle': '要關閉安全模式嗎？',
  'settings.core.safeMode.offBody':
    '確定要關閉安全模式嗎？這樣你就能讓自己以外的使用者執行可能有危險的指令。請確認你信任所有取得這些權限的使用者。',
  'settings.core.turnOff': '關閉',
  'settings.core.codeExecution.label': '程式碼執行',
  'settings.core.codeExecution.hint':
    '巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令。',
  'settings.core.codeExecution.hintJavaOnly': '自訂 Java 指令。',
  'settings.core.codeExecution.locked':
    '已被此核心的管理員鎖定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。',
  'settings.core.codeExecution.offTitle': '要關閉程式碼執行嗎？',
  'settings.core.codeExecution.offBody':
    '執行中的巨集和 Atom 伺服器將會停止。在你重新開啟程式碼執行之前，巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令都不會在此核心上執行。任何項目都不會自動重新啟動。',
  'settings.core.codeExecution.offBodyJavaOnly':
    '在你重新開啟程式碼執行之前，自訂 Java 指令不會在此核心上執行。任何項目都不會自動重新啟動。',
  'settings.core.lanHttp.label': '允許透過未加密 HTTP 進行區域網路存取',
  'settings.core.lanHttp.hint': '未加密。你網路中的其他人可以讀取密碼。',
  'settings.core.lanHttp.confirmTitle': '要允許其他裝置使用未加密 HTTP 嗎？',
  'settings.core.lanHttp.confirmBody':
    '你網路中的裝置將能透過未加密 HTTP 連線。以這種方式傳送的密碼和工作階段權杖會以未加密的方式經過網路，網路上的其他任何人都能讀取並接管此核心上的帳戶。只有在某台裝置無法使用 https:// 時才允許此項。',
  'settings.core.lanHttp.allow': '允許',
  'settings.core.lanHttp.warningOwner':
    '其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請在下方關閉「{setting}」，並改用 https:// 連線。',
  'settings.core.lanHttp.warningOther':
    '其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請使用 https:// 連線，並請擁有者關閉未加密 HTTP 存取。',
  'settings.core.openPort.label': '開放連接埠',
  'settings.core.openPort.hint': '使用 {upnp} 在路由器上設定連接埠轉送。',
  'settings.core.openPort.noPermission': '沒有權限',
  'settings.core.openPort.button': '開放連接埠',
  'settings.core.openPort.portLabel': '要開放的連接埠',
  'settings.core.openPort.portNotNumber': '連接埠必須是數字',
  'settings.core.openPort.portRequired': '請輸入連接埠',
  'settings.core.openPort.portRange': '連接埠必須介於 1 到 65535 之間',
  'settings.core.openPort.opened': '已開放連接埠 {port}',
  'settings.core.info.coreVersion': '核心版本',
  'settings.core.info.dashboardVersion': '控制面板版本',
  'settings.core.info.os': '作業系統',
  'settings.core.info.arch': '架構',
  'settings.core.info.cpu': 'CPU',
  'settings.core.info.cpuCores': 'CPU 核心數',
  'settings.core.info.totalRam': '記憶體總量',
  'settings.core.info.noRam': '沒有記憶體？你是怎麼執行 Lodestone 的？',
  'settings.core.info.httpsCertificate': 'HTTPS 憑證',
  'settings.core.info.fingerprint': '憑證指紋（SHA-256）',
  'settings.core.tls.selfSigned': '內建自我簽署憑證',
  'settings.core.tls.userProvided': '你自己的憑證（tls/cert.pem）',
  'settings.core.tls.none': '無法使用，僅限未加密 HTTP',

  'settings.users.documentTitle': '使用者設定',
  'settings.users.title': '使用者',
  'settings.users.description': '此核心上的帳戶及其權限。',
  'settings.users.allUsers': '所有使用者',
  'settings.users.allMembers': '所有成員',
  'settings.users.allMembersCount': '所有成員（{count}）',
  'settings.users.createUser': '建立新使用者',
  'settings.users.createTitle': '建立新的使用者帳戶',
  'settings.users.createLead': '新使用者初始沒有任何權限。',
  'settings.users.loadingUsers': '正在載入使用者',
  'settings.users.loadingUser': '正在載入該使用者',
  'settings.users.loadingServers': '正在載入伺服器',
  'settings.users.permissions': '權限',
  'settings.users.permissionsLead': '{username} 在此核心上可以執行的操作。',
  'settings.users.unsafePermissions': '不安全的權限',
  'settings.users.unsafePermissionsLead': '關閉安全模式後才能授予非擁有者。',
  'settings.users.ownerOnly': '巨集、擴充功能和 Atom 伺服器',
  'settings.users.ownerOnlyHint': '僅限擁有者。Docker 伺服器使用一般權限。',

  'settings.permission.can_create_instance.title': '建立伺服器',
  'settings.permission.can_create_instance.user':
    '此使用者可以建立新的伺服器。',
  'settings.permission.can_create_instance.key': '此金鑰可以建立新的伺服器。',
  'settings.permission.can_delete_instance.title': '刪除伺服器',
  'settings.permission.can_delete_instance.user':
    '此使用者可以刪除任何伺服器。',
  'settings.permission.can_delete_instance.key': '此金鑰可以刪除任何伺服器。',
  'settings.permission.can_read_global_file.title': '讀取全域檔案',
  'settings.permission.can_read_global_file.user':
    '此使用者可以讀取執行 Lodestone Core 的電腦上的任何檔案。',
  'settings.permission.can_read_global_file.key':
    '此金鑰可以讀取執行 Lodestone Core 的電腦上的任何檔案。',
  'settings.permission.can_view_instance.title': '檢視伺服器',
  'settings.permission.can_view_instance.user': '此使用者可以看到這些伺服器。',
  'settings.permission.can_view_instance.key': '此金鑰可以看到這些伺服器。',
  'settings.permission.can_start_instance.title': '啟動伺服器',
  'settings.permission.can_start_instance.user': '此使用者可以啟動這些伺服器。',
  'settings.permission.can_start_instance.key': '此金鑰可以啟動這些伺服器。',
  'settings.permission.can_stop_instance.title': '停止伺服器',
  'settings.permission.can_stop_instance.user': '此使用者可以停止這些伺服器。',
  'settings.permission.can_stop_instance.key': '此金鑰可以停止這些伺服器。',
  'settings.permission.can_access_instance_console.title': '存取伺服器主控台',
  'settings.permission.can_access_instance_console.user':
    '此使用者可以讀取這些伺服器的主控台並傳送指令。這實際上等於授予伺服器的管理員（OP）權限。',
  'settings.permission.can_access_instance_console.key':
    '此金鑰可以讀取這些伺服器的主控台並傳送指令。這實際上等於授予伺服器的管理員（OP）權限。',
  'settings.permission.can_access_instance_setting.title': '存取伺服器設定',
  'settings.permission.can_access_instance_setting.user':
    '此使用者可以讀取和變更這些伺服器的設定。',
  'settings.permission.can_access_instance_setting.key':
    '此金鑰可以讀取和變更這些伺服器的設定。',
  'settings.permission.can_read_instance_resource.title': '讀取伺服器資源',
  'settings.permission.can_read_instance_resource.user':
    '此使用者可以讀取這些伺服器的資源（世界、模組等）。',
  'settings.permission.can_read_instance_resource.key':
    '此金鑰可以讀取這些伺服器的資源（世界、模組等）。',
  'settings.permission.can_read_instance_file.title': '讀取伺服器檔案',
  'settings.permission.can_read_instance_file.user':
    '此使用者可以讀取這些伺服器的檔案。',
  'settings.permission.can_read_instance_file.key':
    '此金鑰可以讀取這些伺服器的檔案。',
  'settings.permission.can_write_instance_resource.title': '修改伺服器資源',
  'settings.permission.can_write_instance_resource.user':
    '此使用者可以修改這些伺服器的資源（世界、模組等）。',
  'settings.permission.can_write_instance_resource.key':
    '此金鑰可以修改這些伺服器的資源（世界、模組等）。',
  'settings.permission.can_write_instance_file.title': '修改伺服器檔案',
  'settings.permission.can_write_instance_file.user':
    '此使用者可以修改這些伺服器的檔案。',
  'settings.permission.can_write_instance_file.key':
    '此金鑰可以修改這些伺服器的檔案。',
  'settings.permission.can_write_global_file.title': '修改全域檔案',
  'settings.permission.can_write_global_file.user':
    '此使用者可以修改執行 Lodestone Core 的電腦上的任何檔案。',
  'settings.permission.can_write_global_file.key':
    '此金鑰可以修改執行 Lodestone Core 的電腦上的任何檔案。',
  'settings.permission.can_manage_permission.title': '管理權限',
  'settings.permission.can_manage_permission.user':
    '此使用者可以管理其他使用者的權限。',
  'settings.permission.can_manage_permission.key':
    '此金鑰可以管理其他使用者的權限。',

  'settings.profile.documentTitle': '使用者個人資料',
  'settings.profile.title': '個人資料',
  'settings.profile.guest': '訪客',
  'settings.profile.notLoggedIn': '你尚未登入！請登入後再存取此頁面。',
  'settings.profile.account': '帳戶',
  'settings.profile.username': '使用者名稱',
  'settings.profile.password': '密碼',
  'settings.profile.changePassword': '變更密碼',
  'settings.profile.appearance': '外觀',
  'settings.profile.appearanceLead': '儲存在此瀏覽器中，而不是核心上。',
  'settings.profile.coloredConsole': '彩色主控台',

  'settings.form.username': '使用者名稱',
  'settings.form.password': '密碼',
  'settings.form.oldPassword': '舊密碼',
  'settings.form.confirmPassword': '確認密碼',
  'settings.form.usernameRequired': '請輸入使用者名稱',
  'settings.form.passwordRequired': '請輸入密碼',
  'settings.form.confirmRequired': '請確認密碼',
  'settings.form.passwordsMustMatch': '兩次輸入的密碼不一致',
  'settings.form.changePassword': '變更密碼',
  'settings.form.createUser': '建立使用者',

  'settings.minecraft.title': 'Minecraft 帳戶',
  'settings.minecraft.ownLead': '從 Lodestone Launcher 連結。',
  'settings.minecraft.ownEmpty':
    '尚未連結 Minecraft 帳戶。在 Lodestone Launcher 中登入此核心即可連結。',
  'settings.minecraft.userLead':
    '{username} 從 Lodestone Launcher 連結的 Minecraft 帳戶。',
  'settings.minecraft.userEmpty': '{username} 沒有連結的 Minecraft 帳戶。',
  'settings.minecraft.tooOld': '此核心版本太舊，無法連結 Minecraft 帳戶。',
  'settings.minecraft.removeTitle': '移除 Minecraft 帳戶',
  'settings.minecraft.removeBody':
    '要移除與 {name} 的連結嗎？之後可以從 Lodestone Launcher 重新連結。',
  'settings.minecraft.removed': '已移除 {name}',

  'settings.apiKeys.title': 'API 金鑰',
  'settings.apiKeys.ownLead':
    '讓 CLI、指令碼和代理程式無須密碼即可以你的身分操作。',
  'settings.apiKeys.ownEmpty': '你還沒有 API 金鑰。',
  'settings.apiKeys.userLead':
    '{username} 建立的金鑰。撤銷後，使用它的程式會立即失去存取權。',
  'settings.apiKeys.userEmpty': '{username} 沒有 API 金鑰。',
  'settings.apiKeys.create': '建立 API 金鑰',
  'settings.apiKeys.created': '已建立 API 金鑰「{name}」',
  'settings.apiKeys.loading': '正在載入 API 金鑰',
  'settings.apiKeys.revoke': '撤銷',
  'settings.apiKeys.revokeTitle': '撤銷 API 金鑰',
  'settings.apiKeys.revokeBody':
    '要撤銷「{name}」（{display}）嗎？使用此金鑰的所有程式將立即失去存取權。此操作無法復原。',
  'settings.apiKeys.revoked': '已撤銷 API 金鑰「{name}」',
  'settings.apiKeys.createdOn': '建立於 {date}',
  'settings.apiKeys.expiresOn': '{date} 到期',
  'settings.apiKeys.expiredOn': '已於 {date} 到期',
  'settings.apiKeys.expiresNever': '到期時間：永不',
  'settings.apiKeys.lastUsed': '上次使用：{time}',
  'settings.apiKeys.lastUsedNever': '上次使用：從未',
  'settings.apiKeys.scope.full': '完整存取',
  'settings.apiKeys.scope.launcher': 'Launcher：檢視、啟動和停止伺服器',
  'settings.apiKeys.scope.launcherConsole':
    'Launcher：檢視、啟動和停止伺服器，讀取和使用主控台',
  'settings.apiKeys.scope.launcherConsoleRead':
    'Launcher：檢視、啟動和停止伺服器，讀取主控台',
  'settings.apiKeys.scope.launcherConsoleWrite':
    'Launcher：檢視、啟動和停止伺服器，傳送主控台指令',
  'settings.apiKeys.scope.custom': '自訂：{servers}，{permissions}',
  'settings.apiKeys.scope.servers': { other: '{count} 個伺服器' },
  'settings.apiKeys.scope.permissions': { other: '{count} 項權限' },
  'settings.apiKeys.passwordSessionOnly':
    '只有使用密碼登入時才能管理 API 金鑰，使用 API 金鑰登入時無法管理。',
  'settings.apiKeys.createLead':
    '為每個金鑰設定較短的有效期限，並只授予需要的存取權。',
  'settings.apiKeys.name': '名稱',
  'settings.apiKeys.nameRequired': '請輸入名稱',
  'settings.apiKeys.nameTooLong': '名稱最多 {max} 個字元',
  'settings.apiKeys.expiresIn': '有效期限',
  'settings.apiKeys.expiry.30d': '30 天',
  'settings.apiKeys.expiry.90d': '90 天',
  'settings.apiKeys.expiry.1y': '1 年',
  'settings.apiKeys.expiry.never': '永不到期',
  'settings.apiKeys.neverWarning':
    '永不到期的金鑰在你撤銷之前會一直有效。除非確實需要，否則請設定有效期限。',
  'settings.apiKeys.access': '存取權',
  'settings.apiKeys.access.full': '完整存取（你能做的一切）',
  'settings.apiKeys.access.custom': '自訂',
  'settings.apiKeys.chooseOne': '請至少選擇一項權限。',
  'settings.apiKeys.createKey': '建立金鑰',
  'settings.apiKeys.onlyYours':
    '金鑰只能擁有你自己擁有的權限。你沒有的權限和伺服器會顯示為灰色。',
  'settings.apiKeys.notYours': '你沒有此權限。',
  'settings.apiKeys.noServers': '沒有你可以看到的伺服器。',
  'settings.apiKeys.unsafe': '不安全的設定',
  'settings.apiKeys.unsafeSafeMode':
    '安全模式已開啟，因此在核心設定中關閉安全模式之前，核心會拒絕將這些權限授予自訂存取的金鑰。',
  'settings.apiKeys.ownerOnly': '巨集、擴充功能和 Atom 伺服器',
  'settings.apiKeys.ownerOnlyHint':
    '只有完整存取的金鑰才能使用巨集、擴充功能和 Atom 伺服器。',
  'settings.apiKeys.newTitle': '已建立 API 金鑰「{name}」',
  'settings.apiKeys.newCopyNow': '請立即複製此金鑰。之後將無法再次檢視。',
  'settings.apiKeys.newUseWithCli': '在 Lodestone CLI 中使用：',
  'settings.apiKeys.newStore':
    '請像保管密碼一樣保管此金鑰。在它到期或被你撤銷之前，任何擁有它的人都能以它的權限使用 Lodestone。',
  'settings.apiKeys.keyCopied': '已將 API 金鑰複製到剪貼簿',
  'settings.apiKeys.cliCopied': '已將 CLI 範例複製到剪貼簿',
  'settings.apiKeys.copyFailed': '無法複製到剪貼簿。請選取文字並手動複製。',

  // ===== Server pages: tabs, overview, settings, performance, players =====

  // tabList
  'server.tab.overview': '概覽',
  'server.tab.settings': '設定',
  'server.tab.console': '主控台',
  'server.tab.files': '檔案',
  'server.tab.mods': '模組',
  'server.tab.macros': '巨集',
  'server.tab.logs': '事件日誌',

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
  'server.share.title': '與好友分享',
  'server.share.lead': '好友透過連結在 Lodestone Launcher 中加入',
  'server.share.off':
    '把連結傳給好友。他們在 Lodestone Launcher 中開啟後，遊戲會直接連線到這台電腦，無需連接埠轉送。',
  'server.share.start': '與好友分享',
  'server.share.state.starting': '正在啟動…',
  'server.share.state.sharing': '分享中',
  'server.share.state.reconnecting': '正在重新連線…',
  'server.share.stableLink': '重新啟動後連結仍然有效。',
  'server.share.linkLabel': '邀請連結',
  'server.share.copy': '複製連結',
  'server.share.copied': '已複製邀請連結',
  'server.share.copyFailed': '無法複製。請選取連結後手動複製。',
  'server.share.preparing': '正在取得連結…',
  'server.share.howFriendsJoin':
    '伺服器執行期間，任何擁有此連結的人都能加入。沒有 Lodestone Launcher 的好友會看到下載連結。',
  'server.share.hardNetwork':
    '此網路的路由器難以建立直接連線，部分好友可能無法加入。',
  'server.share.connected': { other: '{count} 位好友已連線' },
  'server.share.joins': { other: '（目前共加入 {count} 次）' },
  'server.share.friendLine': '透過{connection}，{ms} 毫秒',
  'server.share.connection.same_computer': '這台電腦',
  'server.share.connection.local_network': '區域網路',
  'server.share.connection.ipv6': 'IPv6',
  'server.share.connection.internet': '網際網路',
  'server.share.stop': '停止分享',
  'server.share.stopTitle': '停止分享？',
  'server.share.stopBody':
    '正在遊戲的好友會中斷連線，連結也將失效。再次分享會產生新連結。',
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

  // InstanceCard：啟動未接受 Minecraft EULA 的伺服器
  'server.eula.title': '接受 Minecraft EULA',
  'server.eula.body': '你同意 {link} 後，{name} 才能啟動。你同意嗎？',
  'server.eula.accept': '我同意，啟動伺服器',
  'server.eula.acceptFailed': '無法接受 Minecraft EULA：{error}',

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

  // ===== Console and macros =====

  // GameConsole: a server's console tab
  'console.pageTitle': '伺服器主控台',
  'console.searchPlaceholder': '搜尋主控台…',
  'console.regex': '規則運算式',
  'console.status.noPermission': '沒有存取主控台的權限',
  'console.status.loading': '主控台載入中…',
  'console.status.buffered': '歷史訊息，沒有即時更新',
  'console.status.live': '主控台已即時連線',
  'console.status.liveNoBuffer':
    '主控台已即時連線，但無法取得歷史記錄。你的網路連線可能不穩定',
  'console.status.closed': '主控台已關閉',
  'console.status.error': '連線中斷或發生錯誤',
  'console.serverStopped': '伺服器已停止',
  'console.input.noPermission': '沒有權限',
  'console.input.placeholder': '輸入指令…',
  'console.noPermission': '你沒有存取此主控台的權限',
  'console.loading': '主控台載入中',
  'console.empty': '主控台還沒有訊息',
  'console.hiddenLines': {
    other: '已隱藏 {count} 行較早的內容（顯示最新的 {max} 行）',
  },
  'console.sendFailed': '無法傳送指令：{error}',

  // ConsoleLine: one line of console output
  'console.line.linkedUser': '連結了此 Minecraft 帳戶的 Lodestone 使用者',

  // CodeExecution: owner-only features and the code execution switch
  'console.ownerOnlyAtom': '只有擁有者可以控制 atom 伺服器',
  'console.ownerOnlyAtomSentence': '只有擁有者可以控制 atom 伺服器。',
  'console.commandLine.readOnly': '此核心已停用程式碼執行，此設定為唯讀。',
  'console.commandLine.ownerOnly': '只有擁有者可以變更此設定。',

  // Macros page
  'console.macros.pageTitle': '伺服器巨集',
  'console.macros.intro': '你的伺服器的所有巨集',
  'console.macros.ownerOnly': '只有此核心的擁有者可以使用巨集。',
  'console.macros.codeExecutionDisabledBanner':
    '此核心已停用程式碼執行。在核心設定中重新啟用之前，無法執行或設定巨集。',
  'console.macros.codeExecutionDisabled': '此核心已停用程式碼執行',
  'console.macros.tab.macros': '所有巨集',
  'console.macros.tab.tasks': '執行中的工作',
  'console.macros.tab.history': '歷史記錄',
  'console.macros.column.name': '巨集名稱',
  'console.macros.column.lastRun': '上次執行',
  'console.macros.column.created': '建立時間',
  'console.macros.column.finished': '完成時間',
  'console.macros.column.pid': '處理程序 ID',
  'console.macros.noTime': '無',
  'console.macros.run': '執行巨集',
  'console.macros.editConfig': '編輯設定',
  'console.macros.killTask': '強制停止工作',
  'console.macros.configTitle': '巨集設定',
  'console.macros.configRequired': '此巨集需要額外的設定。',
  'console.macros.noServer': '未選取伺服器',
  'console.macros.loadFailed': '無法載入巨集：{error}',
  'console.macros.openConfigFailed': '開啟巨集設定時發生錯誤：{error}',
  'console.macros.runFailed': '執行巨集時發生錯誤：{error}',
  'console.macros.killFailed': '無法強制停止工作：{error}',
  'console.macros.saveConfigFailed': '無法儲存巨集設定：{error}',
  'console.macros.invalidState': '錯誤：狀態無效',

  // MacroCreateForm (not used yet)
  'console.macros.create.name': '巨集名稱',
  'console.macros.create.args': '引數',
  'console.macros.create.submit': '建立巨集',
  'console.macros.create.nameRequired': '必須填寫巨集名稱',

  // ===== Creating a server =====

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
  'create.eula.agree': '我同意 {link}',
  'create.eula.link': 'Minecraft EULA',
  'create.eula.required':
    '只有在你同意 Minecraft EULA 後，Minecraft 伺服器才能執行。',

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
  'create.gameType.new': '新功能',
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

  // ===== File manager =====

  // FileViewer: the page, its toolbar and dialogs
  'files.pageTitle': '伺服器檔案管理員',
  'files.dropToUpload': '放開以上傳檔案',
  'files.toolbar.addRemove': '新增/移除',
  'files.toolbar.selectAll': '全選',
  'files.toolbar.deselectAll': '取消全選',
  'files.toolbar.cutSelected': '剪下所選項目',
  'files.toolbar.downloadSelected': '下載所選項目',
  'files.toolbar.unarchiveSelected': '解壓縮所選項目',
  'files.toolbar.deleteSelected': '刪除所選項目',
  'files.toolbar.paste': { other: '貼上 {count} 個檔案' },
  'files.toolbar.discardChanges': '捨棄變更',
  'files.toolbar.upload': '上傳',
  'files.cutToClipboard': { other: '已剪下 {count} 個檔案到剪貼簿' },
  'files.zippingForDownload': '正在壓縮資料夾以供下載…',
  'files.downloadFailed': '下載 {name} 失敗：{error}',
  'files.onlyFilesUnzip': '只能解壓縮檔案',
  'files.onlyFilesUpload': '只能上傳檔案',
  'files.delete.title': { other: '刪除 {count} 個檔案' },
  'files.delete.message': '確定要刪除以下項目嗎？',
  'files.delete.directory': '（資料夾）',
  'files.delete.unknownType': '（未知類型）',
  'files.loadingFile': '正在載入檔案',
  'files.editor.title': '檔案編輯器',
  'files.editor.selectFile': '選擇一個檔案以檢視其內容',
  'files.noReadPermission': '你沒有讀取此資料夾的權限',
  'files.status.selected': { other: '已選取 {count} 個項目' },
  'files.status.inClipboard': { other: '剪貼簿中有 {count} 個項目' },

  // FileList: the list of files and folders
  'files.list.loading': '正在載入檔案列表',
  'files.list.name': '名稱',
  'files.list.dateModified': '修改日期',
  'files.list.size': '大小',
  'files.list.sortBy': '依{field}排序',
  'files.list.sortedAscending': '依{field}遞增排序',
  'files.list.sortedDescending': '依{field}遞減排序',
  'files.list.empty': '這裡沒有檔案…',
  'files.list.unknownTime': '建立時間未知',
  'files.list.folderIcon': '資料夾圖示',
  'files.list.fileIcon': '檔案圖示',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': '剪下',
  'files.contextMenu.paste': '貼上',
  'files.contextMenu.pasteInto': '貼上到 {name}',
  'files.contextMenu.zip': '壓縮',
  'files.contextMenu.zipTo': '壓縮為 {archive}',
  'files.contextMenu.zipFile': '將 {name} 壓縮為 {archive}',
  'files.contextMenu.zipFiles': { other: '將 {count} 個檔案壓縮為 {archive}' },
  'files.contextMenu.unzipHere': '解壓縮到這裡',
  'files.contextMenu.unzipHereSmart': '智慧解壓縮到這裡',
  'files.contextMenu.unzipTo': '解壓縮到 {name}',
  'files.contextMenu.copiedToClipboard': {
    other: '已複製 {count} 個項目到剪貼簿',
  },
  'files.contextMenu.cutToClipboard': {
    other: '已剪下 {count} 個項目到剪貼簿',
  },
  'files.contextMenu.unsupportedZip': '不支援的壓縮檔類型',
  'files.newFolder': '新增資料夾',
  'files.newFile': '新增檔案',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': '為檔案命名',
  'files.createFile.placeholder': '未命名',
  'files.createFile.submit': '建立檔案',
  'files.createFolder.label': '為資料夾命名',
  'files.createFolder.placeholder': '未命名資料夾',
  'files.createFolder.submit': '建立資料夾',
  'files.rename.label': '為檔案命名',
  'files.rename.submit': '重新命名檔案',

  // FileEditor
  'files.editor.loading': '正在載入編輯器',

  // ===== Mods =====

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
};
