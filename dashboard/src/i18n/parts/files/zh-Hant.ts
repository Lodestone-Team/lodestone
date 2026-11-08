import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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
} satisfies CatalogOf<typeof en>;
