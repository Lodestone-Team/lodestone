import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
  'create.gameType.docker': '了解 Docker 服务器（测试版）',
  'create.gameType.new': '新功能',
  'create.gameType.fetchFailed': '获取服务器出错',
  'create.gameType.modpack': 'Modrinth 整合包',
  'create.gameType.modpackDescription': '从 Modrinth 整合包创建 Fabric 服务器',

  // GameTypeMappings: games' titles and descriptions
  'create.game.genericTitle': '{name}（通用）',
  'create.game.vanillaDescription': 'Mojang 官方的标准原版 Minecraft 服务器。',
  'create.game.forgeDescription':
    '模组框架，可用于安装模组并自定义你的 Minecraft 体验。',
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
    '搜索 Modrinth、粘贴 Modrinth 链接，或上传 .mrpack 文件。仅支持 Fabric 整合包。',
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
} satisfies CatalogOf<typeof en>;
