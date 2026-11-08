import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
  'files.rename.label': '为文件命名',
  'files.rename.submit': '重命名文件',

  // FileEditor
  'files.editor.loading': '正在加载编辑器',
} satisfies CatalogOf<typeof en>;
