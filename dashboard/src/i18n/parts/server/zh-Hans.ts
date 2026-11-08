import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // tabList
  'server.tab.overview': '概览',
  'server.tab.settings': '设置',
  'server.tab.console': '控制台',
  'server.tab.files': '文件',
  'server.tab.mods': '模组',
  'server.tab.macros': '宏',
  'server.tab.logs': '事件日志',
  'server.tab.playitgg': 'Playitgg',

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
  'server.card.noScripting':
    '此核心构建时未包含脚本支持（Deno），因此无法使用 atom 服务器。',
  'server.card.codeExecutionDisabled': '此核心已禁用代码执行',
  'server.card.copyIp': '复制 IP',
  'server.card.online': '{players}/{max} 在线',

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
} satisfies CatalogOf<typeof en>;
