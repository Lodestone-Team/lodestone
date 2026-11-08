import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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

  // CodeExecution: owner-only features and the code execution switch
  'console.ownerOnlyAtom': '只有所有者可以控制 atom 服务器',
  'console.ownerOnlyAtomSentence': '只有所有者可以控制 atom 服务器。',
  'console.commandLine.readOnly': '此核心已禁用代码执行，此设置为只读。',
  'console.commandLine.ownerOnly': '只有所有者可以更改此设置。',

  // Macros page
  'console.macros.pageTitle': '服务器宏',
  'console.macros.intro': '你的服务器的所有宏',
  'console.macros.ownerOnly': '只有此核心的所有者可以使用宏。',
  'console.macros.codeExecutionDisabledBanner':
    '此核心已禁用代码执行。在核心设置中重新启用之前，无法运行或配置宏。',
  'console.macros.codeExecutionDisabled': '此核心已禁用代码执行',
  'console.macros.tab.macros': '所有宏',
  'console.macros.tab.tasks': '运行中的任务',
  'console.macros.tab.history': '历史记录',
  'console.macros.column.name': '宏名称',
  'console.macros.column.lastRun': '上次运行',
  'console.macros.column.created': '创建时间',
  'console.macros.column.finished': '完成时间',
  'console.macros.column.pid': '进程 ID',
  'console.macros.noTime': '无',
  'console.macros.run': '运行宏',
  'console.macros.editConfig': '编辑配置',
  'console.macros.killTask': '强制停止任务',
  'console.macros.configTitle': '宏配置',
  'console.macros.configRequired': '此宏需要额外的配置。',
  'console.macros.noServer': '未选择服务器',
  'console.macros.loadFailed': '无法加载宏：{error}',
  'console.macros.openConfigFailed': '打开宏配置时出错：{error}',
  'console.macros.runFailed': '运行宏时出错：{error}',
  'console.macros.killFailed': '无法强制停止任务：{error}',
  'console.macros.saveConfigFailed': '无法保存宏配置：{error}',
  'console.macros.invalidState': '错误：状态无效',

  // MacroCreateForm (not used yet)
  'console.macros.create.name': '宏名称',
  'console.macros.create.args': '参数',
  'console.macros.create.submit': '创建宏',
  'console.macros.create.nameRequired': '必须填写宏名称',

  // PlayitggOverview: the Playit integration page
  'console.playit.loading': '正在加载 playit.gg',
  'console.playit.loadFailed': '无法加载 playit.gg 状态：{error}',
  'console.playit.signupFailed': '无法生成注册链接：{error}',
  'console.playit.verifyFailed': '无法验证 playit.gg 账户：{error}',
  'console.playit.startFailed': '无法启动 playit.gg 运行程序：{error}',
  'console.playit.stopFailed': '无法停止 playit.gg 运行程序：{error}',
  'console.playit.setupTitle': '设置 Playitgg',
  'console.playit.setupIntro':
    '你需要先注册 Playitgg 才能使用此功能。使用下面的按钮生成注册链接！',
  'console.playit.generateLink': '生成链接',
  'console.playit.setupFollowLink':
    '打开下面的链接并创建账户！创建账户并进入控制面板后，点击下面的“验证”。',
  'console.playit.verify': '验证',
  'console.playit.runnerTitle': 'Playitgg 运行程序',
  'console.playit.runner.Stopped': '已停止',
  'console.playit.runner.Loading': '加载中',
  'console.playit.runner.Started': '已启动',
  'console.playit.trouble':
    '如果遇到问题，请确认已在网站上正确设置隧道，并且{running}！',
  'console.playit.troubleRunning': '你的服务器确实正在运行',
  'console.playit.signupRequired': '你需要先注册 Playitgg 才能使用此功能。',
  'console.playit.signupLink': '点击此处注册。',
  'console.playit.tunnels': '隧道',
  'console.playit.refreshTunnels': '刷新隧道',
  'console.playit.tunnelsIntro':
    '你的隧道列在下面。你可以在 {link} 网站的控制面板中创建更多隧道并更改设置。',
  'console.playit.loadingTunnels': '正在加载隧道',
  'console.playit.localServer': '本地服务器地址：{address}',
  'console.playit.copyAddress': '复制地址',
  'console.playit.noTunnels': '你还没有创建任何隧道',
  'console.playit.notSignedUp': '你还没有注册',
  'console.playit.help': '需要帮助？',
} satisfies CatalogOf<typeof en>;
