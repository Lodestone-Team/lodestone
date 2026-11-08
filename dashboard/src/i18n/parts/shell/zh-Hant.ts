import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
  'shell.event.playitStarted': 'Playitgg 執行器已啟動！',
  'shell.event.playitLoading': 'Playitgg 執行器載入中…',
  'shell.event.playitStopped': 'Playitgg 執行器已停止！',
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
} satisfies CatalogOf<typeof en>;
