import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': '語言',
  'settings.language.hint':
    '控制面板的顯示語言。來自伺服器和核心的名稱、日誌和訊息會維持原樣。',
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
  'settings.core.description': '這些設定作用於核心本身。',
  'settings.core.section.core': '核心',
  'settings.core.section.public': '公開伺服器',
  'settings.core.section.publicLead':
    '任何人無須登入即可看到的內容。只有你可以變更。',
  'settings.core.section.info': '資訊',
  'settings.core.section.infoLead': '關於你的核心和控制面板的資訊。',
  'settings.core.section.danger': '危險區域',
  'settings.core.section.dangerLead':
    '這些設定可能會對你的核心造成無法復原的損害。',
  'settings.core.name.label': '核心名稱',
  'settings.core.name.hint':
    '此核心的暱稱。你和其他人連線到此核心時會看到這個名稱',
  'settings.core.name.empty': '名稱不能是空白',
  'settings.core.name.tooLong': '名稱不能超過 {max} 個字元',
  'settings.core.domain.label': '公開網域或 IP',
  'settings.core.domain.hint': '此核心的網域或公開 IP 位址',
  'settings.core.domain.tooLong': '網域不能超過 {max} 個字元',
  'settings.core.domain.tooltip':
    '如果你已設定連接埠轉送，這裡填入你的公開 IP。否則請維持不變。',
  'settings.core.playit.label': '啟用 Playit 整合',
  'settings.core.playit.hint':
    '啟用 Playit 整合，讓使用者無須連接埠轉送即可將伺服器開放給外部存取',
  'settings.core.listPublicly.label': '公開列出伺服器',
  'settings.core.listPublicly.hint':
    '讓任何能連到此核心的人無須登入即可看到你在下方設為公開的伺服器，且只顯示你為每個伺服器選擇的內容。關閉此項會立即隱藏所有伺服器。Lodestone Launcher 透過它加入這些伺服器。',
  'settings.core.safeMode.label': '安全模式',
  'settings.core.safeMode.hint':
    '盡量防止非擁有者的使用者取得你電腦上的管理權限',
  'settings.core.safeMode.offTitle': '要關閉安全模式嗎？',
  'settings.core.safeMode.offBody':
    '確定要關閉安全模式嗎？這樣你就能讓自己以外的使用者執行可能有危險的指令。請確認你信任所有取得這些權限的使用者。',
  'settings.core.turnOff': '關閉',
  'settings.core.codeExecution.label': '程式碼執行',
  'settings.core.codeExecution.hint':
    '允許巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令在此核心上執行。',
  'settings.core.codeExecution.hintJavaOnly':
    '允許自訂 Java 指令在此核心上執行。',
  'settings.core.codeExecution.locked':
    '已被此核心的管理員鎖定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。',
  'settings.core.codeExecution.offTitle': '要關閉程式碼執行嗎？',
  'settings.core.codeExecution.offBody':
    '執行中的巨集和 Atom 伺服器將會停止。在你重新開啟程式碼執行之前，巨集、Atom 伺服器、擴充功能、伺服器指令碼和自訂 Java 指令都不會在此核心上執行。任何項目都不會自動重新啟動。',
  'settings.core.codeExecution.offBodyJavaOnly':
    '在你重新開啟程式碼執行之前，自訂 Java 指令不會在此核心上執行。任何項目都不會自動重新啟動。',
  'settings.core.lanHttp.label': '允許透過未加密 HTTP 進行區域網路存取',
  'settings.core.lanHttp.hint':
    '讓其他裝置使用 http:// 而不是 https:// 連線。密碼和工作階段權杖會以未加密的方式經過你的網路。本機一律可以使用 http://。',
  'settings.core.lanHttp.confirmTitle': '要允許其他裝置使用未加密 HTTP 嗎？',
  'settings.core.lanHttp.confirmBody':
    '你網路中的裝置將能透過未加密 HTTP 連線。以這種方式傳送的密碼和工作階段權杖會以未加密的方式經過網路，網路上的其他任何人都能讀取並接管此核心上的帳戶。只有在某台裝置無法使用 https:// 時才允許此項。',
  'settings.core.lanHttp.allow': '允許',
  'settings.core.lanHttp.warningOwner':
    '其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請在下方關閉「{setting}」，並改用 https:// 連線。',
  'settings.core.lanHttp.warningOther':
    '其他裝置可以透過未加密 HTTP 連線到此核心，因此密碼和工作階段權杖可能以未加密的方式經過你的網路。請使用 https:// 連線，並請擁有者關閉未加密 HTTP 存取。',
  'settings.core.openPort.label': '開放連接埠',
  'settings.core.openPort.hint':
    '嘗試使用 {upnp} 在你的路由器上設定連接埠轉送。這樣網際網路上的人就能連線到你的伺服器。',
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
  'settings.users.description': '此核心上的帳戶，以及每個帳戶可以執行的操作。',
  'settings.users.allUsers': '所有使用者',
  'settings.users.allMembers': '所有成員',
  'settings.users.allMembersCount': '所有成員（{count}）',
  'settings.users.allMembersLead': '所有使用者的清單。點擊使用者即可管理。',
  'settings.users.createUser': '建立新使用者',
  'settings.users.createTitle': '建立新的使用者帳戶',
  'settings.users.createLead':
    '此使用者一開始沒有任何權限。你可以稍後再授予權限。',
  'settings.users.loadingUsers': '正在載入使用者',
  'settings.users.loadingUser': '正在載入該使用者',
  'settings.users.loadingServers': '正在載入伺服器',
  'settings.users.permissions': '權限',
  'settings.users.permissionsLead': '{username} 在此核心上可以執行的操作。',
  'settings.users.unsafePermissions': '不安全的權限',
  'settings.users.unsafePermissionsLead':
    '在核心設定中關閉安全模式後，才能將這些權限授予非擁有者的使用者。',
  'settings.users.ownerOnly': '巨集、擴充功能和 Atom 伺服器',
  'settings.users.ownerOnlyHint':
    '只有擁有者可以使用巨集、安裝擴充功能，以及建立或控制 Atom 伺服器。這些權限無法授予其他使用者。Docker 伺服器使用一般的伺服器權限。',

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
  'settings.profile.description': '{username} 的個人資料偏好設定',
  'settings.profile.guest': '訪客',
  'settings.profile.notLoggedIn': '你尚未登入！請登入後再存取此頁面。',
  'settings.profile.account': '帳戶',
  'settings.profile.username': '使用者名稱',
  'settings.profile.password': '密碼',
  'settings.profile.changePassword': '變更密碼',
  'settings.profile.appearance': '外觀',
  'settings.profile.appearanceLead': '儲存在此瀏覽器中，而不是核心上。',
  'settings.profile.coloredConsole': '彩色主控台',
  'settings.profile.coloredConsoleHint':
    '在伺服器主控台中顯示伺服器輸出的顏色和樣式',

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
  'settings.minecraft.ownLead':
    '你從 Lodestone Launcher 連結的帳戶，Lodestone Launcher 會透過 Microsoft 確認這些帳戶屬於你。控制面板會在它們旁邊顯示你的名稱。',
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
    '金鑰讓 CLI、指令碼和代理程式無須你的密碼即可以你的身分使用 Lodestone。撤銷金鑰不會讓你在其他地方登出。',
  'settings.apiKeys.ownEmpty': '你還沒有 API 金鑰。',
  'settings.apiKeys.userLead':
    '{username} 建立的金鑰。撤銷金鑰可立即切斷使用它的程式的存取。只有 {username} 可以為自己建立金鑰。',
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
    'CLI、指令碼和代理程式可以使用 API 金鑰代替你的密碼登入。為每個金鑰取個名稱，設定較短的有效期限，並只授予它需要的存取權。',
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
    '自訂存取的金鑰不能使用巨集、安裝擴充功能，也不能建立或控制 Atom 伺服器，即使它們屬於擁有者。完整存取的金鑰可以，前提是你本人可以。',
  'settings.apiKeys.newTitle': '已建立 API 金鑰「{name}」',
  'settings.apiKeys.newCopyNow': '請立即複製此金鑰。之後將無法再次檢視。',
  'settings.apiKeys.newUseWithCli': '在 Lodestone CLI 中使用：',
  'settings.apiKeys.newStore':
    '請像保管密碼一樣保管此金鑰。在它到期或被你撤銷之前，任何擁有它的人都能以它的權限使用 Lodestone。',
  'settings.apiKeys.keyCopied': '已將 API 金鑰複製到剪貼簿',
  'settings.apiKeys.cliCopied': '已將 CLI 範例複製到剪貼簿',
  'settings.apiKeys.copyFailed': '無法複製到剪貼簿。請選取文字並手動複製。',
} satisfies CatalogOf<typeof en>;
