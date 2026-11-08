import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': '语言',
  'settings.language.hint':
    '控制面板的显示语言。来自服务器和核心的名称、日志和消息将保持原样。',
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
  'settings.core.description': '这些设置作用于核心本身。',
  'settings.core.section.core': '核心',
  'settings.core.section.public': '公开服务器',
  'settings.core.section.publicLead':
    '任何人无需登录即可看到的内容。只有你可以更改。',
  'settings.core.section.info': '信息',
  'settings.core.section.infoLead': '关于你的核心和控制面板的信息。',
  'settings.core.section.danger': '危险区域',
  'settings.core.section.dangerLead':
    '这些设置可能会对你的核心造成不可逆的损害。',
  'settings.core.name.label': '核心名称',
  'settings.core.name.hint':
    '此核心的昵称。你和其他人连接到此核心时会看到这个名称',
  'settings.core.name.empty': '名称不能为空',
  'settings.core.name.tooLong': '名称不能超过 {max} 个字符',
  'settings.core.domain.label': '公网域名或 IP',
  'settings.core.domain.hint': '此核心的域名或公网 IP 地址',
  'settings.core.domain.tooLong': '域名不能超过 {max} 个字符',
  'settings.core.domain.tooltip':
    '如果你已设置端口转发，这里填写你的公网 IP。否则请保持不变。',
  'settings.core.playit.label': '启用 Playit 集成',
  'settings.core.playit.hint':
    '启用 Playit 集成，让用户无需端口转发即可将服务器开放给公网访问',
  'settings.core.listPublicly.label': '公开列出服务器',
  'settings.core.listPublicly.hint':
    '让任何能访问此核心的人无需登录即可看到你在下方设为公开的服务器，且只显示你为每个服务器选择的内容。关闭此项会立即隐藏所有服务器。Lodestone Launcher 通过它加入这些服务器。',
  'settings.core.safeMode.label': '安全模式',
  'settings.core.safeMode.hint': '尽量防止非所有者用户获得你计算机上的管理权限',
  'settings.core.safeMode.offTitle': '关闭安全模式？',
  'settings.core.safeMode.offBody':
    '确定要关闭安全模式吗？这样你就可以让自己以外的用户运行可能有危险的命令。请确保你信任所有获得这些权限的用户。',
  'settings.core.turnOff': '关闭',
  'settings.core.codeExecution.label': '代码执行',
  'settings.core.codeExecution.hint':
    '允许宏、Atom 服务器、扩展、服务器脚本和自定义 Java 命令在此核心上运行。',
  'settings.core.codeExecution.hintJavaOnly':
    '允许自定义 Java 命令在此核心上运行。',
  'settings.core.codeExecution.locked':
    '已被此核心的管理员锁定（LODESTONE_DISABLE_CODE_EXECUTION 或 --disable-code-execution）。',
  'settings.core.codeExecution.offTitle': '关闭代码执行？',
  'settings.core.codeExecution.offBody':
    '正在运行的宏和 Atom 服务器将被停止。在你重新开启代码执行之前，宏、Atom 服务器、扩展、服务器脚本和自定义 Java 命令都不会在此核心上运行。任何内容都不会自动重新启动。',
  'settings.core.codeExecution.offBodyJavaOnly':
    '在你重新开启代码执行之前，自定义 Java 命令不会在此核心上运行。任何内容都不会自动重新启动。',
  'settings.core.lanHttp.label': '允许通过明文 HTTP 进行局域网访问',
  'settings.core.lanHttp.hint':
    '让其他设备使用 http:// 而不是 https:// 连接。密码和会话令牌将以未加密的方式经过你的网络。本机始终可以使用 http://。',
  'settings.core.lanHttp.confirmTitle': '允许其他设备使用明文 HTTP？',
  'settings.core.lanHttp.confirmBody':
    '你网络中的设备将能够通过明文 HTTP 连接。以这种方式发送的密码和会话令牌会以未加密的方式经过网络，网络中的其他任何人都可以读取它们并接管此核心上的账户。仅在某台设备无法使用 https:// 时才允许此项。',
  'settings.core.lanHttp.allow': '允许',
  'settings.core.lanHttp.warningOwner':
    '其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请在下方关闭“{setting}”，并改用 https:// 连接。',
  'settings.core.lanHttp.warningOther':
    '其他设备可以通过明文 HTTP 连接到此核心，因此密码和会话令牌可能以未加密的方式经过你的网络。请使用 https:// 连接，并请所有者关闭明文 HTTP 访问。',
  'settings.core.openPort.label': '开放端口',
  'settings.core.openPort.hint':
    '尝试使用 {upnp} 在你的路由器上设置端口转发。这样互联网上的人就可以连接到你的服务器。',
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
  'settings.users.description': '此核心上的账户以及每个账户可以执行的操作。',
  'settings.users.allUsers': '所有用户',
  'settings.users.allMembers': '所有成员',
  'settings.users.allMembersCount': '所有成员（{count}）',
  'settings.users.allMembersLead': '所有用户的列表。点击用户即可管理。',
  'settings.users.createUser': '创建新用户',
  'settings.users.createTitle': '创建新用户账户',
  'settings.users.createLead': '此用户初始没有任何权限。你可以稍后授予权限。',
  'settings.users.loadingUsers': '正在加载用户',
  'settings.users.loadingUser': '正在加载该用户',
  'settings.users.loadingServers': '正在加载服务器',
  'settings.users.permissions': '权限',
  'settings.users.permissionsLead': '{username} 在此核心上可以执行的操作。',
  'settings.users.unsafePermissions': '不安全权限',
  'settings.users.unsafePermissionsLead':
    '在核心设置中关闭安全模式后，才能将这些权限授予非所有者用户。',
  'settings.users.ownerOnly': '宏、扩展和 Atom 服务器',
  'settings.users.ownerOnlyHint':
    '只有所有者可以使用宏、安装扩展以及创建或控制 Atom 服务器。这些权限无法授予其他用户。Docker 服务器使用普通的服务器权限。',

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
  'settings.profile.description': '{username} 的个人资料偏好设置',
  'settings.profile.guest': '访客',
  'settings.profile.notLoggedIn': '你尚未登录！请登录后访问此页面。',
  'settings.profile.account': '账户',
  'settings.profile.username': '用户名',
  'settings.profile.password': '密码',
  'settings.profile.changePassword': '更改密码',
  'settings.profile.appearance': '外观',
  'settings.profile.appearanceLead': '保存在此浏览器中，而不是核心上。',
  'settings.profile.coloredConsole': '彩色控制台',
  'settings.profile.coloredConsoleHint':
    '在服务器控制台中显示服务器输出的颜色和样式',

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
  'settings.minecraft.ownLead':
    '你从 Lodestone Launcher 关联的账户，Lodestone Launcher 会通过 Microsoft 确认这些账户属于你。控制面板会在它们旁边显示你的名字。',
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
    '密钥让 CLI、脚本和智能体无需你的密码即可以你的身份使用 Lodestone。吊销密钥不会让你在其他地方退出登录。',
  'settings.apiKeys.ownEmpty': '你还没有 API 密钥。',
  'settings.apiKeys.userLead':
    '{username} 创建的密钥。吊销密钥可立即切断使用它的程序的访问。只有 {username} 可以为自己创建密钥。',
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
    'CLI、脚本和智能体可以使用 API 密钥代替你的密码登录。为每个密钥起个名字，设置较短的有效期，并只授予它所需的访问权限。',
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
  'settings.apiKeys.ownerOnly': '宏、扩展和 Atom 服务器',
  'settings.apiKeys.ownerOnlyHint':
    '自定义访问的密钥不能使用宏、安装扩展，也不能创建或控制 Atom 服务器，即使它们属于所有者。完全访问的密钥可以，前提是你本人可以。',
  'settings.apiKeys.newTitle': '已创建 API 密钥“{name}”',
  'settings.apiKeys.newCopyNow': '请立即复制此密钥。之后将无法再次查看。',
  'settings.apiKeys.newUseWithCli': '在 Lodestone CLI 中使用：',
  'settings.apiKeys.newStore':
    '请像保管密码一样保管此密钥。在它过期或被你吊销之前，任何拥有它的人都可以凭它的权限使用 Lodestone。',
  'settings.apiKeys.keyCopied': '已将 API 密钥复制到剪贴板',
  'settings.apiKeys.cliCopied': '已将 CLI 示例复制到剪贴板',
  'settings.apiKeys.copyFailed': '无法复制到剪贴板。请选中文本并手动复制。',
} satisfies CatalogOf<typeof en>;
