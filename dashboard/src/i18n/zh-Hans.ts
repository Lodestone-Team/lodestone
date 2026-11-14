// 简体中文目录。键和占位符与 en.ts 相同（由 tsc 和 catalogs.test.ts 检查）。
//
// 风格：中国大陆常用界面用语，称呼用“你”；全角标点；中文与拉丁字母、数字之间
// 加半角空格（如“启动 Minecraft 服务器”）；按钮和标签不加句号。
// 产品名（Lodestone、Lodestone Core、Minecraft、Modrinth）不翻译。
// “服务器”只指游戏服务器；Lodestone Core 称为“核心”，不要称为服务器。
//
// 术语表：
//   server (a game server)           服务器
//   core (Lodestone Core)            核心
//   instance                         实例
//   dashboard                        控制面板
//   console                          控制台
//   settings                         设置
//   user / owner                     用户 / 所有者
//   permission                       权限
//   sign in / sign out               登录 / 退出登录
//   password / username              密码 / 用户名
//   API key / token                  API 密钥 / 令牌
//   file / folder                    文件 / 文件夹
//   upload / download                上传 / 下载
//   mod / modpack                    模组 / 整合包
//   resource pack                    资源包
//   player / world                   玩家 / 世界
//   port / address                   端口 / 地址
//   memory (RAM)                     内存
//   start / stop / restart / kill    启动 / 停止 / 重启 / 强制停止
//   running / stopped                运行中 / 已停止
//   log / event                      日志 / 事件
//   save / delete / remove           保存 / 删除 / 移除
//   enable / disable                 启用 / 禁用
//   connection / network             连接 / 网络
//   default / search                 默认 / 搜索
//   public listing                   公开列表
import type { Messages } from './types.ts';

export const zhHans: Messages = {
  // ===== Common words =====

  'common.cancel': '取消',
  'common.confirm': '确认',
  'common.save': '保存',
  'common.saved': '已保存',
  'common.saving': '正在保存…',
  'common.delete': '删除',
  'common.remove': '移除',
  'common.rename': '重命名',
  'common.edit': '编辑',
  'common.create': '创建',
  'common.add': '添加',
  'common.close': '关闭',
  'common.back': '返回',
  'common.next': '下一步',
  'common.done': '完成',
  'common.copy': '复制',
  'common.copied': '已复制',
  'common.copiedToClipboard': '已复制到剪贴板',
  'common.retry': '重试',
  'common.refresh': '刷新',
  'common.search': '搜索',
  'common.loading': '正在加载…',
  'common.yes': '是',
  'common.no': '否',
  'common.ok': '确定',
  'common.on': '开',
  'common.off': '关',
  'common.enabled': '已启用',
  'common.disabled': '已禁用',
  'common.none': '无',
  'common.unknown': '未知',
  'common.default': '默认',
  'common.optional': '可选',
  'common.required': '必填',
  'common.learnMore': '了解详情',
  'common.showMore': '显示更多',
  'common.showLess': '收起',
  'common.you': '你',
  'common.unknownError': '未知错误',
  'common.networkError': '网络错误',
  'common.networkErrorWithMessage': '网络错误：{message}',
  'common.invalidToken': '令牌无效：请退出登录后重新登录',
  'common.notAnInteger': '不是整数',
  'common.notANumber': '不是有效的数字',
  'common.justNow': '刚刚',
  'common.pageTitle': '{page} - Lodestone',
  'common.state.Starting': '正在启动',
  'common.state.Running': '运行中',
  'common.state.Stopping': '正在停止',
  'common.state.Stopped': '已停止',
  'common.state.Error': '错误',
  // Form fields and settings rows (Atoms/Form, Atoms/Config)
  'common.fieldLabel': '{label}：',
  'common.select': '请选择…',
  'common.noPermission': '无权限',
  'common.input.cannotBeEmpty': '不能为空',
  'common.input.mustBeNumber': '必须是数字',
  'common.input.mustBeGreaterThan': '必须大于 {min}',
  'common.input.mustBeLessThan': '必须小于 {max}',
  'common.combo.addCustom': '添加“{query}”',
  'common.combo.nothingFound': '未找到结果。',
  // Table
  'common.table.noData': '暂无数据',
  // ClipboardTextfield: text that copies itself when clicked
  'common.clipboard.copyToClipboard': '复制到剪贴板',
  'common.clipboard.copiedText': '已复制“{text}”',
  // The server resource pack URL check (utils/resourcePack.ts)
  'common.resourcePack.urlSpaces': 'URL 不能包含空格。',
  'common.resourcePack.notAUrl': '这不是有效的 URL。',
  'common.resourcePack.urlScheme': 'URL 必须以 http:// 或 https:// 开头。',

  // ===== App shell: navigation, top bar, settings frame, sign-in and setup, notifications =====

  'shell.session.expired': '你的会话已过期，请重新登录',
  'shell.session.invalid': '你的会话已失效，请重新登录',
  'shell.token.invalid': '令牌无效',
  'shell.token.invalidExp': '令牌中的 exp 无效',
  'shell.token.invalidUid': '令牌中的 uid 无效',
  'shell.token.expired': '令牌已过期',

  'shell.error.invalidStatusCode': '状态码无效',
  'shell.error.invalidResponse': '响应无效',

  'shell.settings.general': '常规',
  'shell.settings.users': '用户',
  'shell.settings.profile': '个人资料',

  'shell.settings.pagesButton': '设置页面',
  'shell.settings.close': '关闭设置',
  'shell.settings.closeTooltip': '关闭（Esc）',

  'shell.settingsNav.label': '设置',
  'shell.settingsNav.core': '核心',
  'shell.settingsNav.account': '账户',
  'shell.settingsNav.users': '用户',
  'shell.settingsNav.unselectUser': '取消选择用户',

  'shell.beta.title': '感谢使用 Lodestone 测试版！',
  'shell.beta.body':
    '感谢使用 Lodestone 测试版！如果你发现任何问题，请在我们的 Discord 服务器或 Github 上告诉我们。',
  'shell.versionMismatch.title': '需要更新！',
  'shell.versionMismatch.continue': '我已了解，不更新继续',
  'shell.versionMismatch.coreVersion': '核心版本：',
  'shell.versionMismatch.dashboardVersion': '控制面板版本：',
  'shell.versionMismatch.incompatible': '你的控制面板与核心不兼容！',
  'shell.versionMismatch.body':
    '这可能导致意外行为。请将核心更新到最新版本。更多信息请访问 {link}。',
  'shell.versionMismatch.wiki': 'Wiki',
  'shell.versionMismatch.toast':
    '版本不一致！你的核心是否已过时？核心：{core}，控制面板：{dashboard}',
  'shell.localCore.title': '检测到新的本地核心',
  'shell.localCore.body': '检测到一个尚未设置的本地核心。要设置 {name} 吗？',
  'shell.localCore.setup': '设置',
  'shell.localCore.skip': '跳过',
  'shell.setupRequired.title': '需要设置',
  'shell.setupRequired.body': '{name} 尚未设置。请完成设置流程。',
  'shell.setupRequired.changeCore': '更换核心',
  'shell.coreError.title': '核心连接错误',
  'shell.coreError.body':
    '连接 {address} 时出错。请选择其他核心、刷新页面，或等待核心重新上线。',
  'shell.coreError.back': '返回核心选择',
  'shell.coreError.continue': '继续使用当前核心',

  'shell.topNav.degraded': '{refresh}以在 Firefox 上获取最新数据。',
  'shell.topNav.refresh': '刷新',
  'shell.topNav.devBuild': '本地开发版本',

  'shell.connection.connected': '已连接',
  'shell.connection.connectedDetail': '正在接收实时更新',
  'shell.connection.connecting': '正在连接…',
  'shell.connection.connectingDetail': '正在等待核心响应',
  'shell.connection.degraded': '连接受限',
  'shell.connection.degradedDetail': '实时更新不可用，请刷新以更新',
  'shell.connection.disconnected': '已断开',
  'shell.connection.disconnectedDetail': '无法连接到核心',
  'shell.connection.address': '地址',
  'shell.connection.version': '版本',
  'shell.connection.host': '主机',
  'shell.connection.system': '系统',
  'shell.connection.uptime': '运行时间',
  'shell.connection.uptimeDays': '{days} 天 {hours} 小时',
  'shell.connection.uptimeHours': '{hours} 小时 {minutes} 分钟',
  'shell.connection.uptimeMinutes': '{minutes} 分钟',
  'shell.connection.coreFallback': '核心',
  'shell.connection.ariaLabel': '{core}：{status}',

  'shell.nav.allInstances': '所有实例',
  'shell.nav.integrations': '集成',
  'shell.nav.selectedInstance': '已选实例',
  'shell.nav.noSelection': '选中的服务器将显示在这里',
  'shell.nav.loadingServers': '正在加载服务器',
  'shell.nav.logInToView': '登录以查看你的服务器。',
  'shell.nav.newServer': '新建服务器…',
  'shell.nav.noPermission': '没有权限',

  'shell.notifications.title': '通知',
  'shell.notifications.inProgress': '进行中',
  'shell.notifications.all': '所有通知',
  'shell.notifications.loading': '正在加载通知',
  'shell.notifications.empty': '暂无通知！',

  'shell.event.state.Starting': '服务器 {name} 正在启动！',
  'shell.event.state.Running': '服务器 {name} 已启动！',
  'shell.event.state.Stopping': '服务器 {name} 正在停止！',
  'shell.event.state.Stopped': '服务器 {name} 已停止！',
  'shell.event.state.Error': '服务器 {name} 出现错误！',
  'shell.event.warningToast':
    '警告：某个服务器发出了警告。这不应该发生，请向开发者报告此问题。',
  'shell.event.warning': '服务器 {name} 发出了警告',
  'shell.event.error': '服务器 {name} 出现错误',
  'shell.event.playersJoined': '{players} 加入了 {name}',
  'shell.event.playersLeft': '{players} 离开了 {name}',
  'shell.event.playersJoinedAndLeft':
    '{joined} 加入了 {name}，{left} 离开了 {name}',
  'shell.event.modpackUpdate': '{name} 的 {pack} {version} 已发布',
  'shell.event.playerMessage': '{player} 在 {name} 上说：{message}',
  'shell.event.apiKeyCreated': '已创建 API 密钥“{name}”',
  'shell.event.apiKeyRevoked': '已吊销 API 密钥“{name}”',
  'shell.event.loadFailed': '无法加载通知：{error}',

  'shell.userMenu.guest': '访客',
  'shell.userMenu.account': '账户：{name}',
  'shell.userMenu.signedInAs': '已登录为 {name}',
  'shell.userMenu.notSignedIn': '未登录',
  'shell.userMenu.signOut': '退出登录',
  'shell.userMenu.signIn': '登录',
  'shell.userMenu.changeCore': '更换核心',
  'shell.userMenu.settings': '设置',

  'shell.userBox.changePassword': '修改密码',
  'shell.userBox.cantDeleteSelfTitle': '你不能删除自己',
  'shell.userBox.cantDeleteSelf': '你不能删除自己。',
  'shell.userBox.deleteTitle': '删除用户',
  'shell.userBox.deleteConfirm': '确定要删除 {name} 吗？此操作无法撤销。',
  'shell.userBox.deleteUser': '删除用户',
  'shell.userBox.admin': '管理员',
  'shell.userBox.owner': '所有者',

  'shell.connectionHelp.trustCertificate':
    '如果核心正在运行并使用内置证书，此浏览器需要信任该证书一次。打开 {link}，接受证书警告，然后返回重试。',
  'shell.connectionHelp.fingerprint':
    '为确认这是你的核心，请将浏览器显示的证书指纹与核心日志或核心设置中的指纹进行比对。',
  'shell.connectionHelp.plainHttp':
    '默认情况下，Lodestone 核心会拒绝来自其他设备的纯 HTTP 连接。',
  'shell.connectionHelp.savedCore':
    '请使用相同地址和 https 协议连接一个新核心。',
  'shell.connectionHelp.changeProtocol': '请将协议改为 https 后重试。',

  'shell.login.checkFields': '{error}：请确认各字段填写正确。',
  'shell.login.invalidResponse': '响应无效，设置可能有误',
  'shell.login.downloadCore': '下载 Lodestone Core',
  'shell.login.continue': '继续',
  'shell.login.or': '或',
  'shell.login.username': '用户名',
  'shell.login.password': '密码',
  'shell.login.usernameRequired': '请输入用户名',
  'shell.login.passwordRequired': '请输入密码',
  'shell.login.changeCore': '更换核心',
  'shell.login.signIn': '登录',
  'common.localOnly':
    '这个核心只允许它所在电脑上的应用使用。请在那台电脑上打开 Lodestone Host 来管理它，或在那里设置所有者账户（设置 › 常规 › 允许其他设备访问）以使用网页控制台。',
  'shell.localOnly.title': '这个核心只允许它所在的电脑使用',
  'shell.login.localOnly':
    '这个核心只允许这台电脑上的应用使用。请打开 Lodestone Host 来管理它，或在那里设置所有者密码（设置 › 常规 › 允许其他设备访问）以使用网页控制台。',

  'shell.login.withApiKey': '使用 API 密钥登录',
  'shell.login.withPassword': '使用用户名和密码登录',
  'shell.login.apiKey': 'API 密钥',
  'shell.login.apiKeyHint':
    '粘贴以 lsk_ 开头的密钥。你将获得该密钥的权限，撤销密钥后会退出登录。',
  'shell.login.apiKeyRequired': '需要 API 密钥',
  'shell.login.apiKeyFormat': 'API 密钥以 lsk_ 开头',
  'shell.coreConnect.pageTitle': '连接到核心',
  'shell.coreConnect.title': '添加新核心',
  'shell.coreConnect.httpErrorTitle': 'HTTP 错误',
  'shell.coreConnect.mixedContent':
    '缺少网站权限。请允许混合内容，或停用所有 HTTPS 浏览器插件，然后刷新页面。{link}',
  'shell.coreConnect.httpsWarning': '你可能需要调整网络和浏览器设置。{link}',
  'shell.coreConnect.apiVersion': 'API 版本',
  'shell.coreConnect.protocol': '协议',
  'shell.coreConnect.port': '端口',
  'shell.coreConnect.address': 'IP 地址/域名',
  'shell.coreConnect.addressPlaceholder':
    '例如 123.123.123.123 或“myserver.com”',
  'shell.coreConnect.submit': '添加并继续',

  'shell.coreSelect.pageTitle': '选择核心',
  'shell.coreSelect.title': '选择 Lodestone Core',
  'shell.coreSelect.existing': '选择已有核心',
  'shell.coreSelect.connectNew': '连接新核心',

  'shell.coreSetup.pageTitle': '设置新核心',
  'shell.coreSetup.title': '创建所有者账户',
  'shell.coreSetup.findKey': '请在核心的控制台输出中查找“First time setup key”',
  'shell.coreSetup.setupKey': '设置密钥',
  'shell.coreSetup.setupKeyPrefilled': '设置密钥（已预填）',
  'shell.coreSetup.confirmPassword': '确认密码',
  'shell.coreSetup.submit': '提交',
  'shell.coreSetup.passwordConfirmRequired': '请确认密码',
  'shell.coreSetup.passwordMismatch': '两次输入的密码不一致',
  'shell.coreSetup.setupKeyRequired': '请输入设置密钥',
  'shell.coreSetup.failed': '设置核心时出错',

  'shell.coreConfig.pageTitle': '配置新核心',
  'shell.coreConfig.title': '自定义你的核心',
  'shell.coreConfig.coreName': '核心名称',
  'shell.coreConfig.domain': '公网域名/IP（可选）',
  'shell.coreConfig.coreNameRequired': '请输入核心名称',
  'shell.coreConfig.domainTooLong': '域名不能超过 {max} 个字符',

  'shell.welcome.pageTitle': '欢迎使用 Lodestone',
  'shell.welcome.wiki': 'Wiki 和常见问题',
  'shell.welcome.title': '欢迎使用 Lodestone！',
  'shell.welcome.version': '版本 {version}',
  'shell.welcome.beta': 'Lodestone 是测试版软件！',
  'shell.welcome.betaBody':
    '浏览器支持有限，可能会遇到错误。不保证功能的稳定性和未来的兼容性。你可以在我们的 {github} 上查看已知问题并报告新问题。',
  'shell.welcome.connectExisting': '连接到已有核心',
  'shell.welcome.setupCore': '设置 Lodestone Core',

  'shell.userLogin.switchAccount': '切换账户',
  'shell.userSelect.pageTitle': '选择用户',
  'shell.userSelect.loadingUser': '正在加载用户',
  'shell.userSelect.continueAs': '以 {name} 继续',
  'shell.userSelect.continueAsCurrent': '以当前用户继续',
  'shell.userSelect.switchUser': '切换用户',

  'shell.home.pageTitle': '主页',
  'shell.home.title': '管理你的服务器',
  'shell.home.subtitle': '首先，从菜单中选择一个服务器，或新建一个',
  'shell.home.create': '新建服务器',

  'shell.notFound.title': '找不到页面',
  'shell.notFound.goHome': '返回主页',

  // ===== Settings: core settings, users, profile, API keys =====

  'settings.language.label': '语言',
  'settings.language.auto': '跟随浏览器（{language}）',

  'settings.theme.label': '主题',
  'settings.theme.system': '跟随系统',
  'settings.theme.light': '浅色',
  'settings.theme.dark': '深色',
  'settings.theme.systemHint': '跟随设备的浅色或深色设置',
  'settings.theme.lightHint': '始终使用浅色',
  'settings.theme.darkHint': '始终使用深色',

  'settings.toc.label': '章节',

  'settings.legacy.title': '核心设置',

  'settings.core.documentTitle': 'Lodestone Core 设置',
  'settings.core.title': '常规',
  'settings.core.section.core': '核心',
  'settings.core.section.public': '公开服务器',
  'settings.core.section.publicLead': '无需登录即可查看。',
  'settings.core.section.info': '信息',
  'settings.core.section.danger': '危险区域',
  'settings.core.section.dangerLead': '请谨慎更改。',
  'settings.core.name.label': '核心名称',
  'settings.core.name.empty': '名称不能为空',
  'settings.core.name.tooLong': '名称不能超过 {max} 个字符',
  'settings.core.domain.label': '公网域名或 IP',
  'settings.core.domain.tooLong': '域名不能超过 {max} 个字符',
  'settings.core.domain.tooltip':
    '如果你已设置端口转发，这里填写你的公网 IP。否则请保持不变。',
  'settings.core.modpackChecks.label': '检查整合包更新',
  'settings.core.modpackChecks.hint':
    '每隔几小时在 Modrinth 上查找服务器所用整合包的新版本，有新版本时通知。关闭后，服务器“模组”标签中的“检查更新”仍可使用。',
  'settings.core.listPublicly.label': '公开列出服务器',
  'settings.core.listPublicly.hint':
    '任何能访问此核心的人都能看到你在下方设为公开的服务器。',
  'settings.core.localClaim.label': '允许本机上的应用获取所有者权限',
  'settings.core.localClaim.hint':
    'Lodestone Launcher 可以通过证明自己能写入此核心的数据文件夹来成为所有者。能写入该文件夹的程序本来就能修改所有用户和设置，所以这只在共用电脑上有影响。',
  'settings.core.safeMode.label': '安全模式',
  'settings.core.safeMode.hint': '仅所有者可获得此计算机的管理权限。',
  'settings.core.safeMode.offTitle': '关闭安全模式？',
  'settings.core.safeMode.offBody':
    '确定要关闭安全模式吗？这样你就可以让自己以外的用户运行可能有危险的命令。请确保你信任所有获得这些权限的用户。',
  'settings.core.turnOff': '关闭',
  'settings.core.codeExecution.label': '代码执行',
  'settings.core.codeExecution.hint': '自定义 Java 命令。',
  'settings.core.codeExecution.locked':
    '已被此核心的管理员锁定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。',
  'settings.core.codeExecution.offTitle': '关闭代码执行？',
  'settings.core.codeExecution.offBody':
    '在你重新开启代码执行之前，自定义 Java 命令不会在此核心上运行。任何内容都不会自动重新启动。',
  'settings.core.lanHttp.label': '允许通过明文 HTTP 进行局域网访问',
  'settings.core.lanHttp.hint': '未加密。你网络中的其他人可以读取密码。',
  'settings.core.lanHttp.confirmTitle': '允许其他设备使用明文 HTTP？',
  'settings.core.lanHttp.confirmBody':
    '你网络中的设备将能够通过明文 HTTP 连接。以这种方式发送的密码和会话令牌会以未加密的方式经过网络，网络中的其他任何人都可以读取它们并接管此核心上的账户。仅在某台设备无法使用 https:// 时才允许此项。',
  'settings.core.lanHttp.allow': '允许',
  'settings.core.lanHttp.warningOwner':
    '其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请在下方关闭“{setting}”，并改用 https:// 连接。',
  'settings.core.lanHttp.warningOther':
    '其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请使用 https:// 连接，并请所有者关闭明文 HTTP 访问。',
  'settings.core.openPort.label': '开放端口',
  'settings.core.openPort.hint': '使用 {upnp} 在路由器上设置端口转发。',
  'settings.core.openPort.noPermission': '没有权限',
  'settings.core.openPort.button': '开放端口',
  'settings.core.openPort.portLabel': '要开放的端口',
  'settings.core.openPort.portNotNumber': '端口必须是数字',
  'settings.core.openPort.portRequired': '请输入端口',
  'settings.core.openPort.portRange': '端口必须在 1 到 65535 之间',
  'settings.core.openPort.opened': '已开放端口 {port}',
  'settings.core.info.coreVersion': '核心版本',
  'settings.core.info.dashboardVersion': '控制面板版本',
  'settings.core.info.os': '操作系统',
  'settings.core.info.arch': '架构',
  'settings.core.info.cpu': 'CPU',
  'settings.core.info.cpuCores': 'CPU 核心数',
  'settings.core.info.totalRam': '总内存',
  'settings.core.info.noRam': '没有内存？你是怎么运行 Lodestone 的？',
  'settings.core.info.httpsCertificate': 'HTTPS 证书',
  'settings.core.info.fingerprint': '证书指纹（SHA-256）',
  'settings.core.tls.selfSigned': '内置自签名证书',
  'settings.core.tls.userProvided': '你自己的证书（tls/cert.pem）',
  'settings.core.tls.none': '不可用，仅明文 HTTP',

  'settings.users.documentTitle': '用户设置',
  'settings.users.title': '用户',
  'settings.users.description': '此核心上的账户及其权限。',
  'settings.users.allUsers': '所有用户',
  'settings.users.allMembers': '所有成员',
  'settings.users.allMembersCount': '所有成员（{count}）',
  'settings.users.createUser': '创建新用户',
  'settings.users.createTitle': '创建新用户账户',
  'settings.users.createLead': '新用户初始没有任何权限。',
  'settings.users.loadingUsers': '正在加载用户',
  'settings.users.loadingUser': '正在加载该用户',
  'settings.users.loadingServers': '正在加载服务器',
  'settings.users.permissions': '权限',
  'settings.users.permissionsLead': '{username} 在此核心上可以执行的操作。',
  'settings.users.unsafePermissions': '不安全权限',
  'settings.users.unsafePermissionsLead': '关闭安全模式后才能授予非所有者。',

  'settings.permission.can_create_instance.title': '创建服务器',
  'settings.permission.can_create_instance.user': '该用户可以创建新服务器。',
  'settings.permission.can_create_instance.key': '该密钥可以创建新服务器。',
  'settings.permission.can_delete_instance.title': '删除服务器',
  'settings.permission.can_delete_instance.user': '该用户可以删除任何服务器。',
  'settings.permission.can_delete_instance.key': '该密钥可以删除任何服务器。',
  'settings.permission.can_read_global_file.title': '读取全局文件',
  'settings.permission.can_read_global_file.user':
    '该用户可以读取运行 Lodestone Core 的计算机上的任何文件。',
  'settings.permission.can_read_global_file.key':
    '该密钥可以读取运行 Lodestone Core 的计算机上的任何文件。',
  'settings.permission.can_view_instance.title': '查看服务器',
  'settings.permission.can_view_instance.user': '该用户可以看到这些服务器。',
  'settings.permission.can_view_instance.key': '该密钥可以看到这些服务器。',
  'settings.permission.can_start_instance.title': '启动服务器',
  'settings.permission.can_start_instance.user': '该用户可以启动这些服务器。',
  'settings.permission.can_start_instance.key': '该密钥可以启动这些服务器。',
  'settings.permission.can_stop_instance.title': '停止服务器',
  'settings.permission.can_stop_instance.user': '该用户可以停止这些服务器。',
  'settings.permission.can_stop_instance.key': '该密钥可以停止这些服务器。',
  'settings.permission.can_access_instance_console.title': '访问服务器控制台',
  'settings.permission.can_access_instance_console.user':
    '该用户可以读取这些服务器的控制台并向其发送命令。这实际上相当于授予服务器的管理员（OP）权限。',
  'settings.permission.can_access_instance_console.key':
    '该密钥可以读取这些服务器的控制台并向其发送命令。这实际上相当于授予服务器的管理员（OP）权限。',
  'settings.permission.can_access_instance_setting.title': '访问服务器设置',
  'settings.permission.can_access_instance_setting.user':
    '该用户可以读取和更改这些服务器的设置。',
  'settings.permission.can_access_instance_setting.key':
    '该密钥可以读取和更改这些服务器的设置。',
  'settings.permission.can_read_instance_resource.title': '读取服务器资源',
  'settings.permission.can_read_instance_resource.user':
    '该用户可以读取这些服务器的资源（世界、模组等）。',
  'settings.permission.can_read_instance_resource.key':
    '该密钥可以读取这些服务器的资源（世界、模组等）。',
  'settings.permission.can_read_instance_file.title': '读取服务器文件',
  'settings.permission.can_read_instance_file.user':
    '该用户可以读取这些服务器的文件。',
  'settings.permission.can_read_instance_file.key':
    '该密钥可以读取这些服务器的文件。',
  'settings.permission.can_write_instance_resource.title': '修改服务器资源',
  'settings.permission.can_write_instance_resource.user':
    '该用户可以修改这些服务器的资源（世界、模组等）。',
  'settings.permission.can_write_instance_resource.key':
    '该密钥可以修改这些服务器的资源（世界、模组等）。',
  'settings.permission.can_write_instance_file.title': '修改服务器文件',
  'settings.permission.can_write_instance_file.user':
    '该用户可以修改这些服务器的文件。',
  'settings.permission.can_write_instance_file.key':
    '该密钥可以修改这些服务器的文件。',
  'settings.permission.can_write_global_file.title': '修改全局文件',
  'settings.permission.can_write_global_file.user':
    '该用户可以修改运行 Lodestone Core 的计算机上的任何文件。',
  'settings.permission.can_write_global_file.key':
    '该密钥可以修改运行 Lodestone Core 的计算机上的任何文件。',
  'settings.permission.can_manage_permission.title': '管理权限',
  'settings.permission.can_manage_permission.user':
    '该用户可以管理其他用户的权限。',
  'settings.permission.can_manage_permission.key':
    '该密钥可以管理其他用户的权限。',

  'settings.profile.documentTitle': '用户个人资料',
  'settings.profile.title': '个人资料',
  'settings.profile.guest': '访客',
  'settings.profile.notLoggedIn': '你尚未登录！请登录后访问此页面。',
  'settings.profile.account': '账户',
  'settings.profile.username': '用户名',
  'settings.profile.password': '密码',
  'settings.profile.changePassword': '更改密码',
  'settings.profile.appearance': '外观',
  'settings.profile.appearanceLead': '保存在此浏览器中，而不是核心上。',
  'settings.profile.coloredConsole': '彩色控制台',

  'settings.form.username': '用户名',
  'settings.form.password': '密码',
  'settings.form.oldPassword': '旧密码',
  'settings.form.confirmPassword': '确认密码',
  'settings.form.usernameRequired': '请输入用户名',
  'settings.form.passwordRequired': '请输入密码',
  'settings.form.confirmRequired': '请确认密码',
  'settings.form.passwordsMustMatch': '两次输入的密码不一致',
  'settings.form.changePassword': '更改密码',
  'settings.form.createUser': '创建用户',

  'settings.minecraft.title': 'Minecraft 账户',
  'settings.minecraft.ownLead': '从 Lodestone Launcher 关联。',
  'settings.minecraft.ownEmpty':
    '尚未关联 Minecraft 账户。在 Lodestone Launcher 中登录此核心即可关联。',
  'settings.minecraft.userLead':
    '{username} 从 Lodestone Launcher 关联的 Minecraft 账户。',
  'settings.minecraft.userEmpty': '{username} 没有关联的 Minecraft 账户。',
  'settings.minecraft.tooOld': '此核心版本太旧，无法关联 Minecraft 账户。',
  'settings.minecraft.removeTitle': '移除 Minecraft 账户',
  'settings.minecraft.removeBody':
    '移除与 {name} 的关联？之后可以从 Lodestone Launcher 重新关联。',
  'settings.minecraft.removed': '已移除 {name}',

  'settings.apiKeys.title': 'API 密钥',
  'settings.apiKeys.ownLead':
    '让 CLI、脚本和智能体无需密码即可以你的身份操作。',
  'settings.apiKeys.ownEmpty': '你还没有 API 密钥。',
  'settings.apiKeys.userLead':
    '{username} 创建的密钥。吊销后，使用它的程序会立即失去访问权限。',
  'settings.apiKeys.userEmpty': '{username} 没有 API 密钥。',
  'settings.apiKeys.create': '创建 API 密钥',
  'settings.apiKeys.created': '已创建 API 密钥“{name}”',
  'settings.apiKeys.loading': '正在加载 API 密钥',
  'settings.apiKeys.revoke': '吊销',
  'settings.apiKeys.revokeTitle': '吊销 API 密钥',
  'settings.apiKeys.revokeBody':
    '吊销“{name}”（{display}）？使用此密钥的所有程序将立即失去访问权限。此操作无法撤销。',
  'settings.apiKeys.revoked': '已吊销 API 密钥“{name}”',
  'settings.apiKeys.createdOn': '创建于 {date}',
  'settings.apiKeys.expiresOn': '{date} 过期',
  'settings.apiKeys.expiredOn': '已于 {date} 过期',
  'settings.apiKeys.expiresNever': '过期时间：永不',
  'settings.apiKeys.lastUsed': '上次使用：{time}',
  'settings.apiKeys.lastUsedNever': '上次使用：从未',
  'settings.apiKeys.scope.full': '完全访问',
  'settings.apiKeys.scope.launcher': 'Launcher：查看、启动和停止服务器',
  'settings.apiKeys.scope.launcherConsole':
    'Launcher：查看、启动和停止服务器，读取和使用控制台',
  'settings.apiKeys.scope.launcherConsoleRead':
    'Launcher：查看、启动和停止服务器，读取控制台',
  'settings.apiKeys.scope.launcherConsoleWrite':
    'Launcher：查看、启动和停止服务器，发送控制台命令',
  'settings.apiKeys.scope.custom': '自定义：{servers}，{permissions}',
  'settings.apiKeys.scope.servers': { other: '{count} 个服务器' },
  'settings.apiKeys.scope.permissions': { other: '{count} 项权限' },
  'settings.apiKeys.passwordSessionOnly':
    '只有使用密码登录时才能管理 API 密钥，使用 API 密钥登录时不行。',
  'settings.apiKeys.createLead':
    '为每个密钥设置较短的有效期，并只授予所需的访问权限。',
  'settings.apiKeys.name': '名称',
  'settings.apiKeys.nameRequired': '请输入名称',
  'settings.apiKeys.nameTooLong': '名称最多 {max} 个字符',
  'settings.apiKeys.expiresIn': '有效期',
  'settings.apiKeys.expiry.30d': '30 天',
  'settings.apiKeys.expiry.90d': '90 天',
  'settings.apiKeys.expiry.1y': '1 年',
  'settings.apiKeys.expiry.never': '永不过期',
  'settings.apiKeys.neverWarning':
    '永不过期的密钥在你吊销之前一直有效。除非确实需要，否则请设置有效期。',
  'settings.apiKeys.access': '访问权限',
  'settings.apiKeys.access.full': '完全访问（你能做的一切）',
  'settings.apiKeys.access.custom': '自定义',
  'settings.apiKeys.chooseOne': '请至少选择一项权限。',
  'settings.apiKeys.createKey': '创建密钥',
  'settings.apiKeys.onlyYours':
    '密钥只能拥有你自己拥有的权限。你没有的权限和服务器显示为灰色。',
  'settings.apiKeys.notYours': '你没有此权限。',
  'settings.apiKeys.noServers': '没有你可以看到的服务器。',
  'settings.apiKeys.unsafe': '不安全设置',
  'settings.apiKeys.unsafeSafeMode':
    '安全模式已开启，因此在核心设置中关闭安全模式之前，核心会拒绝为自定义访问的密钥授予这些权限。',
  'settings.apiKeys.newTitle': '已创建 API 密钥“{name}”',
  'settings.apiKeys.newCopyNow': '请立即复制此密钥。之后将无法再次查看。',
  'settings.apiKeys.newUseWithCli': '在 Lodestone CLI 中使用：',
  'settings.apiKeys.newStore':
    '请像保管密码一样保管此密钥。在它过期或被你吊销之前，任何拥有它的人都可以凭它的权限使用 Lodestone。',
  'settings.apiKeys.keyCopied': '已将 API 密钥复制到剪贴板',
  'settings.apiKeys.cliCopied': '已将 CLI 示例复制到剪贴板',
  'settings.apiKeys.copyFailed': '无法复制到剪贴板。请选中文本并手动复制。',

  // ===== Server pages: tabs, overview, settings, performance, players =====

  // tabList
  'server.tab.overview': '概览',
  'server.tab.settings': '设置',
  'server.tab.console': '控制台',
  'server.tab.files': '文件',
  'server.tab.mods': '模组',
  'server.tab.logs': '事件日志',

  // InstanceTabs
  'server.notFound': '找不到服务器',
  'server.tabNotFound': '找不到 {path}',
  'server.logs.comingSoon': '即将推出，敬请期待！',

  // InstanceOverview
  'server.overview.pageTitle': '服务器概览',
  'server.overview.noName': '无名称',
  'server.overview.noDescription': '无描述',
  'server.overview.version': '版本 {version}',
  'server.overview.playerCount': '玩家数 {players}/{max}',

  // InstanceDetails
  'server.details.title': '详细信息',
  'server.details.name': '名称',
  'server.details.description': '描述',
  'server.details.uuid': 'UUID',
  'server.details.game': '游戏',
  'server.details.minecraftVersion': 'Minecraft 版本',
  'server.details.version': '版本',
  'server.details.loader': '加载器',
  'server.details.port': '端口',
  'server.details.state': '状态',
  'server.details.players': '玩家',
  'server.details.created': '创建时间',
  'server.details.autoStart': '自动启动',
  'server.details.restartOnCrash': '崩溃后重启',
  'server.details.ram': '最小 / 最大内存',
  'server.details.ramValue': '{min} MB / {max} MB',
  'server.details.folder': '文件夹',
  'server.details.modpack': '创建自整合包',

  // InstancePerformance
  'server.performance.title': '服务器监控',
  'server.performance.lead': '此实例对你的 Lodestone Core 的资源占用',
  'server.performance.cpuLoad': 'CPU 负载：{value}%',
  'server.performance.memoryUsed': '已用内存：{value} GiB',
  'server.performance.loadingGraph': '正在加载图表',
  'server.performance.cpuUsage': 'CPU 使用率',
  'server.performance.memoryUsage': '内存使用量',
  'server.performance.unitPercent': '%',
  'server.performance.unitGiB': ' GiB',

  // PerformanceGraph
  'server.graph.seconds': '{seconds} 秒',
  'server.graph.tooltipTime': '{time}：',

  // InstancePlayerList
  'server.players.title': '玩家列表',
  'server.players.lead': '当前在线的所有玩家',
  'server.players.sortByName': '名称',
  'server.players.avatarAlt': '{name} 的头像',
  'server.players.linkedUser': '关联此 Minecraft 账户的 Lodestone 用户',
  'server.players.none': '没有在线玩家',

  // InstanceSettings
  'server.settings.pageTitle': '服务器设置',
  'server.settings.automation': '自动化',
  'server.settings.automationLead': 'Lodestone 何时自动启动此服务器。',
  'server.settings.autoStart': '自动启动',
  'server.settings.autoStartHint': 'Lodestone Core 启动时启动此服务器',
  'server.settings.restartOnCrash': '崩溃后重启',
  'server.settings.restartOnCrashHint': '此服务器崩溃时重新启动它',
  'server.settings.notAvailable': '暂时不可用',
  'server.settings.notAvailableHint': '请至少启动此服务器一次',
  'server.settings.noPermission': '没有权限',
  'server.settings.dangerZone': '危险区域',
  'server.settings.dangerZoneLead':
    '这些设置可能对你的服务器造成不可逆的损害。',
  'server.settings.deleteServer': '删除服务器',
  'server.settings.deleteServerHint': '永久删除此服务器及其数据',
  'server.settings.deleteTitle': '删除“{name}”',
  'server.settings.deleteConfirm': '我明白，删除此服务器',
  'server.settings.deleteCannotUndo': '此操作无法撤销。',
  'server.settings.deleteWarning':
    '{cannotUndo}此实例的设置、世界和备份将被永久删除。继续之前，请备份所有重要数据。',

  // SettingField
  'server.settingField.unknownType': '提交错误：未知的值类型。',

  // PublicListingSetting
  'server.share.title': '共享',
  'server.share.lead': '拥有链接的人可在 Lodestone Launcher 中加入',
  'server.share.off':
    '把链接发给你想一起玩的人。他们在 Lodestone Launcher 中打开后，游戏会直接连接到这台电脑，无需端口转发。',
  'server.share.start': '共享',
  'server.share.state.starting': '正在启动…',
  'server.share.state.sharing': '共享中',
  'server.share.state.reconnecting': '正在重新连接…',
  'server.share.stableLink': '重启后链接仍然有效。',
  'server.share.linkLabel': '邀请链接',
  'server.share.copy': '复制链接',
  'server.share.copied': '已复制邀请链接',
  'server.share.copyFailed': '无法复制。请选中链接后手动复制。',
  'server.share.preparing': '正在获取链接…',
  'server.share.howToJoin':
    '服务器运行期间，任何拥有此链接的人都能加入。没有 Lodestone Launcher 的人会看到下载链接。',
  'server.share.hardNetwork':
    '此网络的路由器难以建立直接连接，部分玩家可能无法加入。',
  'server.share.connected': { other: '{count} 人已连接' },
  'server.share.joins': { other: '（目前共加入 {count} 次）' },
  'server.share.peerLine': '通过{connection}，{ms} 毫秒',
  'server.share.connection.same_computer': '这台电脑',
  'server.share.connection.local_network': '局域网',
  'server.share.connection.ipv6': 'IPv6',
  'server.share.connection.internet': '互联网',
  'server.share.stop': '停止共享',
  'server.share.stopTitle': '停止共享？',
  'server.share.stopBody':
    '通过链接游戏的人都会断开连接，链接也将失效。再次共享会生成新链接。',
  'server.publicListing.title': '共享',
  'server.publicListing.lead': '谁可以在不登录的情况下找到此服务器。',
  'server.publicListing.listedPublicly': '公开列出',
  'server.publicListing.public': '公开',
  'server.publicListing.ownerOnly': '只有所有者可以更改此项',
  'server.publicListing.flag.status': '状态和玩家',
  'server.publicListing.flag.address': '加入地址',
  'server.publicListing.flag.clientPack': '客户端模组下载',
  'server.publicListing.shownText':
    '任何能访问此核心的人无需登录即可看到此服务器的{list}。',
  'server.publicListing.shown.nameVersion': '名称和版本',
  'server.publicListing.shown.status': '状态和玩家数',
  'server.publicListing.shown.address': '地址',
  'server.publicListing.shown.clientPack': '客户端模组',
  'server.publicListing.turnOnFirst': '请先开启“公开列出服务器”。',
  'server.publicListing.hiddenWhileOff': '{shown}“公开列出服务器”关闭时隐藏。',
  'server.publicListing.notShown': '未登录时不显示。',
  'server.publicListing.noServers': '还没有 Minecraft Java Edition 服务器。',

  // ResourcePackSetting
  'server.resourcePack.title': '资源包',
  'server.resourcePack.lead': '玩家加入时会被要求下载的资源包。',
  'server.resourcePack.url': 'URL',
  'server.resourcePack.urlHint': '资源包 zip 的直接链接。留空则不使用资源包。',
  'server.resourcePack.sha1Hint': '让玩家可以校验下载内容并缓存资源包。',
  'server.resourcePack.sha1Error': 'SHA-1 应为 40 个十六进制字符。',
  'server.resourcePack.compute': '计算',
  'server.resourcePack.downloaded': '已下载 {size}。',
  'server.resourcePack.required': '强制使用资源包',
  'server.resourcePack.requiredHint': '拒绝资源包的玩家会被断开连接。',
  'server.resourcePack.prompt': '提示信息',
  'server.resourcePack.promptHint':
    '询问玩家时显示的消息。纯文本或 JSON 文本组件。',
  'server.resourcePack.remove': '移除资源包',
  'server.resourcePack.saved': '资源包已保存',
  'server.resourcePack.savedRestart': '资源包已保存。重启服务器后生效。',

  // InstanceCard
  'server.action.start': '启动',
  'server.action.stop': '停止',
  'server.action.restart': '重启',
  'server.action.kill': '强制停止',
  'server.card.power': '电源',
  'server.card.copyIp': '复制 IP',
  'server.card.share': '共享',
  'server.card.shareHint': '通过链接共享此服务器',
  'server.card.online': '{players}/{max} 在线',

  // InstanceCard：启动未接受 Minecraft EULA 的服务器
  'server.eula.title': '接受 Minecraft EULA',
  'server.eula.body': '你同意 {link} 后，{name} 才能启动。你同意吗？',
  'server.eula.accept': '我同意，启动服务器',
  'server.eula.acceptFailed': '无法接受 Minecraft EULA：{error}',

  // InstanceLoadingPill
  'server.loading.settingUp': '正在设置…（{percent}）',

  // apis.ts
  'server.api.deletedFile': '已删除文件：{name}',
  'server.api.deletedDirectory': '已删除文件夹：{path}',
  'server.api.downloadFailed': '下载 {name} 失败：{error}',
  'server.api.uploading': {
    other: '正在上传 {count} 个文件',
  },
  'server.api.moveFailed': '移动 {name} 失败：{error}',
  'server.api.zipFailed': '无法开始压缩：{error}',
  'server.api.zipping': {
    other: '正在压缩 {count} 个项目…',
  },
  'server.api.unzipFailed': '解压 {name} 失败：{error}',
  'server.api.unzipping': '正在解压 {name}…',
  'server.api.tooManyAttemptsSeconds': {
    other: '登录失败次数过多。请等待 {count} 秒后重试。',
  },
  'server.api.tooManyAttemptsMinutes': {
    other: '登录失败次数过多。请等待 {count} 分钟后重试。',
  },
  'server.api.tooManyAttempts': '登录失败次数过多。请等待几分钟后重试。',
  'server.api.loginFailed': '登录失败：{message}',
  'server.api.loginError': '错误：{message}',

  // ===== Console =====

  // GameConsole: a server's console tab
  'console.pageTitle': '服务器控制台',
  'console.searchPlaceholder': '搜索控制台…',
  'console.regex': '正则表达式',
  'console.status.noPermission': '没有访问控制台的权限',
  'console.status.loading': '正在加载控制台…',
  'console.status.buffered': '历史消息，没有实时更新',
  'console.status.live': '控制台已实时连接',
  'console.status.liveNoBuffer':
    '控制台已实时连接，但无法获取历史记录。你的网络连接可能不稳定',
  'console.status.closed': '控制台已关闭',
  'console.status.error': '连接已断开或出错',
  'console.serverStopped': '服务器已停止',
  'console.input.noPermission': '没有权限',
  'console.input.placeholder': '输入命令…',
  'console.noPermission': '你没有访问此控制台的权限',
  'console.loading': '正在加载控制台',
  'console.empty': '控制台还没有消息',
  'console.hiddenLines': {
    other: '已隐藏 {count} 行较早的内容（显示最新的 {max} 行）',
  },
  'console.sendFailed': '无法发送命令：{error}',

  // ConsoleLine: one line of console output
  'console.line.linkedUser': '关联了此 Minecraft 账户的 Lodestone 用户',

  // CodeExecution: the custom Java command settings and the code execution switch
  'console.commandLine.readOnly': '此核心已禁用代码执行，此设置为只读。',
  'console.commandLine.ownerOnly': '只有所有者可以更改此设置。',

  // ===== Creating a server =====

  // InstanceCreateForm / ModpackCreateForm: the steps in the dialog's sidebar
  'create.step.selectGame': '选择游戏',
  'create.step.basicSettings': '基本设置',
  'create.step.serverSettings': '服务器设置',
  'create.step.autoSettings': '自动化设置',
  'create.step.selectModpack': '选择整合包',
  'create.step.selectVersion': '选择版本',

  // InstanceCreateForm: the pages' subtitles, buttons and errors
  'create.basicSettings.description': '服务器的基本设置。',
  'create.serverSettings.description': '配置你的服务器。',
  'create.autoSettings.description': '自动配置你的服务器。',
  'create.checkFields': '请确保所有字段都已正确填写',
  'create.createServer': '创建服务器',
  'create.createFailed': '创建服务器出错：{error}',
  'create.unknownValueType': '生成表单出错：未知的值类型。',
  'create.eula.agree': '我同意 {link}',
  'create.eula.link': 'Minecraft EULA',
  'create.eula.required':
    '只有在你同意 Minecraft EULA 后，Minecraft 服务器才能运行。',

  // form.ts and ModpackCreateForm: the dashboard's own setup fields
  'create.field.name': '名称',
  'create.field.nameDescription': '服务器的名称',
  'create.field.nameDefault': '我的服务器',
  'create.field.description': '描述',
  'create.field.descriptionDescription': '服务器的描述',
  'create.field.port': '端口',
  'create.field.portDescription': '服务器运行的端口',
  'create.field.minRam': '最小内存',
  'create.field.minRamDescription': '分配给服务器的最小内存，单位为 MB',
  'create.field.maxRam': '最大内存',
  'create.field.maxRamDescription': '分配给服务器的最大内存，单位为 MB',
  'create.field.autoStart': '自动启动',
  'create.field.autoStartDescription': '应用程序启动时自动启动服务器',
  'create.field.restartOnCrash': '崩溃后重启',
  'create.field.restartOnCrashDescription': '服务器崩溃后自动重启',

  // form.ts: validation of a setting; {name} is the setting's name (often sent by the core, untranslated)
  'create.validation.required': '“{name}”为必填项',
  'create.validation.number': '“{name}”必须是数字',
  'create.validation.integer': '“{name}”必须是整数',
  'create.validation.positive': '“{name}”必须为正数',
  'create.validation.min': '“{name}”必须大于或等于 {min}',
  'create.validation.max': '“{name}”必须小于或等于 {max}',
  'create.validation.oneOf': '“{name}”必须是可用选项之一',

  // FormComboField: the options list of a field
  'create.combo.add': '添加“{query}”',
  'create.combo.nothingFound': '未找到任何结果',

  // GameTypeSelectForm: the first step, picking a game
  'create.gameType.title': '选择游戏',
  'create.gameType.subtitle': '你的服务器将用于什么？',
  'create.gameType.new': '新功能',
  'create.gameType.modpack': 'Modrinth 整合包',
  'create.gameType.modpackDescription':
    '从 Modrinth 整合包创建 Fabric、Forge 或 NeoForge 服务器',

  // GameTypeMappings: games' titles and descriptions
  'create.game.genericTitle': '{name}（通用）',
  'create.game.vanillaDescription': 'Mojang 官方的标准原版 Minecraft 服务器。',
  'create.game.forgeDescription':
    '模组框架，可用于安装模组并自定义你的 Minecraft 体验。',
  'create.game.neoforgeDescription':
    '社区维护的 Forge 分支，有自己的模组，适用于 Minecraft 1.20.2 及更高版本。',
  'create.game.fabricDescription': '轻量级的 Minecraft 模组工具链。',
  'create.game.paperDescription':
    '高性能的 Spigot 分支，旨在修复游戏玩法和机制上的不一致。',
  'create.game.spigotDescription':
    '经过修改的 Minecraft 服务器软件，支持插件，提供更高的性能和更多自定义选项。',
  'create.game.otherDescription': '未知的 Minecraft 变体：{name}',
  'create.game.bedrockDescription': 'Minecraft Bedrock Edition 服务器。',
  'create.game.genericDescription': '未知游戏：{name}',

  // CreateInstanceFlow: read by screen readers while the dialog loads
  'create.loadingForm': '正在加载创建表单',

  // ModpackCreateForm: creating a server from a Modrinth modpack
  'create.modpack.sourceTitle': '选择整合包',
  'create.modpack.sourceDescription':
    '搜索 Modrinth、粘贴 Modrinth 链接，或上传 .mrpack 文件。支持 Fabric、Forge 和 NeoForge 整合包。',
  'create.modpack.searchPlaceholder': '搜索整合包，或粘贴 Modrinth 链接',
  'create.modpack.onlyFabric': '仅显示 Fabric 整合包',
  'create.modpack.upload': '上传 .mrpack',
  'create.modpack.versionTitle': '选择版本',
  'create.modpack.versionUploaded': '已上传的整合包。',
  'create.modpack.versionDescription': '选择要安装的整合包版本。',
  'create.modpack.downloading': '正在下载整合包文件以读取其内容…',
  'create.modpack.downloadingSize': '正在下载整合包文件（{size}）以读取其内容…',
  'create.modpack.versionHint':
    '选择一个版本以查看其内容。Lodestone 会下载该版本的整合包文件进行读取；在你点击“创建服务器”之前，不会安装任何内容。',
  'create.modpack.settingsTitle': '服务器设置',
  'create.modpack.loadingModpack': '正在加载整合包',
  'create.modpack.loadingVersions': '正在加载版本',
  'create.modpack.versionsFailed': '加载版本失败：{error}',
  'create.modpack.noVersions': '此整合包没有任何版本。',
  'create.modpack.select': '选择',
  'create.modpack.selected': '已选择',
  'create.modpack.showMore': {
    other: '显示更多（还剩 {count} 个）',
  },
  // the summary of the picked version
  'create.modpack.summary.modpack': '整合包',
  'create.modpack.summary.loader': '加载器',
  'create.modpack.summary.serverFiles': '服务器文件',
  'create.modpack.summary.serverFilesValue': '{count} 个（需下载 {size}）',
  'create.modpack.unsupported': '无法使用此整合包：{reason}',
  // toasts and errors
  'create.modpack.inspectFailed': '读取整合包失败：{error}',
  'create.modpack.notAModpack': '这是 {type}，不是整合包',
  'create.modpack.chooseMrpack': '请选择 .mrpack 文件',
  'create.modpack.noUploadId': '核心未返回上传 ID',
  'create.modpack.uploadFailed': '上传整合包失败：{error}',
  // validation of the settings step
  'create.modpack.nameRequired': '名称为必填项',
  'create.modpack.portNumber': '端口必须是数字',
  'create.modpack.portInteger': '端口必须是整数',
  'create.modpack.portRange': '端口必须介于 1 到 65535 之间',
  'create.modpack.portRequired': '端口为必填项',
  'create.modpack.minRamNumber': '最小内存必须是数字',
  'create.modpack.minRamInteger': '最小内存必须是整数',
  'create.modpack.minRamMin': '最小内存至少为 1 MB',
  'create.modpack.minRamRequired': '最小内存为必填项',
  'create.modpack.maxRamNumber': '最大内存必须是数字',
  'create.modpack.maxRamInteger': '最大内存必须是整数',
  'create.modpack.maxRamMin': '最大内存不能小于最小内存',
  'create.modpack.maxRamRequired': '最大内存为必填项',

  // ===== File manager =====

  // FileViewer: the page, its toolbar and dialogs
  'files.pageTitle': '服务器文件管理器',
  'files.dropToUpload': '松开以上传文件',
  'files.toolbar.addRemove': '添加/移除',
  'files.toolbar.selectAll': '全选',
  'files.toolbar.deselectAll': '取消全选',
  'files.toolbar.cutSelected': '剪切所选项',
  'files.toolbar.downloadSelected': '下载所选项',
  'files.toolbar.unarchiveSelected': '解压所选项',
  'files.toolbar.deleteSelected': '删除所选项',
  'files.toolbar.paste': { other: '粘贴 {count} 个文件' },
  'files.toolbar.discardChanges': '放弃更改',
  'files.toolbar.upload': '上传',
  'files.cutToClipboard': { other: '已剪切 {count} 个文件到剪贴板' },
  'files.zippingForDownload': '正在压缩文件夹以供下载…',
  'files.downloadFailed': '下载 {name} 失败：{error}',
  'files.onlyFilesUnzip': '只能解压文件',
  'files.onlyFilesUpload': '只能上传文件',
  'files.delete.title': { other: '删除 {count} 个文件' },
  'files.delete.message': '确定要删除以下项目吗？',
  'files.delete.directory': '（文件夹）',
  'files.delete.unknownType': '（未知类型）',
  'files.loadingFile': '正在加载文件',
  'files.editor.title': '文件编辑器',
  'files.editor.selectFile': '选择一个文件以查看其内容',
  'files.noReadPermission': '你没有读取此文件夹的权限',
  'files.status.selected': { other: '已选择 {count} 个项目' },
  'files.status.inClipboard': { other: '剪贴板中有 {count} 个项目' },

  // FileList: the list of files and folders
  'files.list.loading': '正在加载文件列表',
  'files.list.empty': '这里没有文件…',
  'files.list.unknownTime': '创建时间未知',
  'files.list.folderIcon': '文件夹图标',
  'files.list.fileIcon': '文件图标',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': '剪切',
  'files.contextMenu.paste': '粘贴',
  'files.contextMenu.pasteInto': '粘贴到 {name}',
  'files.contextMenu.zip': '压缩',
  'files.contextMenu.zipTo': '压缩为 {archive}',
  'files.contextMenu.zipFile': '将 {name} 压缩为 {archive}',
  'files.contextMenu.zipFiles': { other: '将 {count} 个文件压缩为 {archive}' },
  'files.contextMenu.unzipHere': '解压到此处',
  'files.contextMenu.unzipHereSmart': '智能解压到此处',
  'files.contextMenu.unzipTo': '解压到 {name}',
  'files.contextMenu.copiedToClipboard': {
    other: '已复制 {count} 个项目到剪贴板',
  },
  'files.contextMenu.cutToClipboard': {
    other: '已剪切 {count} 个项目到剪贴板',
  },
  'files.contextMenu.unsupportedZip': '不支持的压缩文件类型',
  'files.newFolder': '新建文件夹',
  'files.newFile': '新建文件',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': '为文件命名',
  'files.createFile.placeholder': '未命名',
  'files.createFile.submit': '创建文件',
  'files.createFolder.label': '为文件夹命名',
  'files.createFolder.placeholder': '未命名文件夹',
  'files.createFolder.submit': '创建文件夹',
  'files.nameTaken': '此文件夹中已有 {name}',
  'files.rename.label': '为文件命名',
  'files.rename.submit': '重命名文件',

  // FileEditor
  'files.editor.loading': '正在加载编辑器',

  // ===== Mods =====

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
  'mods.clientRequirement.unknownModrinthMark':
    'Modrinth 未说明玩家是否需要它，请查看该模组的页面。如果玩家需要，请勾选“给玩家”。',
  'mods.clientRequirement.unknownNotOnModrinthMark':
    '不在 Modrinth 上，请查看该模组的页面。如果玩家需要，请勾选“给玩家”。',
  'mods.forPlayers.label': '给玩家',
  'mods.forPlayers.hint':
    '玩家加入时会获得这个模组及其依赖的模组。对于 Lodestone 无法判断但玩家需要的模组（例如你上传的），请勾选此项。',
  'mods.forPlayers.already':
    '玩家已会获得这个模组：Modrinth 或模组本身表明玩家需要它。',
  'mods.forPlayers.marked': '玩家下次加入时会获得 {name}',
  'mods.forPlayers.unmarked': '{name} 不再标记为给玩家',
  'mods.forPlayers.failed': '无法更改 {name}：{error}',
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

  'mods.modpack.from': '此服务器使用整合包 {pack}。',
  'mods.modpack.check': '检查更新',
  'mods.modpack.checkFailed': '无法检查更新：{error}',
  'mods.modpack.upToDate': '已是最新版本。',
  'mods.modpack.whatsNew': '更新内容',
  'mods.modpack.out': '{version} 已发布',
  'mods.modpack.updateTo': '更新到 {version}',
  'mods.modpack.fromFile': '从文件更新',
  'mods.modpack.notMrpack': '{name} 不是 .mrpack 文件。',
  'mods.modpack.planFailed': '无法准备更新：{error}',
  'mods.modpack.history': '历史',
  'mods.modpack.historyFailed': '无法加载整合包的历史：{error}',
  'mods.modpack.historyEmpty': '尚无记录。之后的更新可以撤销。',
  'mods.modpack.kind.install': '已安装',
  'mods.modpack.kind.baseline': '首次记录',
  'mods.modpack.kind.update': '已更新',
  'mods.modpack.kind.restore': '已恢复 {version}',
  'mods.modpack.current': '当前',
  'mods.modpack.restore': '恢复',
  'mods.modpack.restoreMissing': 'Lodestone 已不再保有此版本的部分文件。',
  'mods.modpack.restoreTitle': '恢复此版本？',
  'mods.modpack.updateTitle': '更新整合包？',
  'mods.modpack.update': '更新',
  'mods.modpack.restored': '已恢复 {pack} {version}。',
  'mods.modpack.updated': '已将 {pack} 更新到 {version}。',
  'mods.modpack.restoreFailed': '无法恢复整合包：{error}',
  'mods.modpack.updateFailed': '无法更新整合包：{error}',
  'mods.modpack.storeSize':
    '为了撤销更新，Lodestone 保留了 {size} 无法重新下载的文件。',
  'mods.modpack.since': '自 {version} 起',
  'mods.modpack.sinceTooltip': '此文件随整合包 {version} 而来。',
  'mods.modpack.unknownVersion': '未知版本',
  'mods.modpack.dialog.intro': '{pack} 将从 {from} 更新到 {to}。',
  'mods.modpack.dialog.reinstall': '服务器将改用 {software} 并重新安装。',
  'mods.modpack.dialog.kept': '你修改过的文件将保持不变：{files}',
  'mods.modpack.dialog.nothing': '没有文件会改变。',
  'mods.modpack.dialog.stop': '请先停止服务器：服务器运行时无法更新。',
  'mods.modpack.dialog.unsupported': '无法使用此版本：{reason}',
  'mods.modpack.added': { other: '已添加 {count} 个文件' },
  'mods.modpack.replaced': { other: '已替换 {count} 个文件' },
  'mods.modpack.removed': { other: '已删除 {count} 个文件' },
  'mods.modpack.keptCount': { other: '保留了 {count} 个已修改的文件' },
  'mods.modpack.moreChanges': { other: '还有 {count} 个' },

  // ClientPackPanel
  'mods.clientPack.loader': 'Fabric Loader {version}',
  'mods.clientPack.loaderNoVersion': 'Fabric Loader',
  'mods.clientPack.plainFabric':
    '玩家只需原版 Fabric 即可加入（Minecraft {gameVersion}，{loader}）。',
  'mods.clientPack.title': { other: '玩家需要 {count} 个模组才能加入' },
  'mods.clientPack.download': '下载客户端整合包（.mrpack）',
  'mods.clientPack.showAll': {
    one: '显示全部 {count} 个模组',
    other: '显示全部 {count} 个模组',
  },
  'mods.clientPack.showFewer': '收起',
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
};
