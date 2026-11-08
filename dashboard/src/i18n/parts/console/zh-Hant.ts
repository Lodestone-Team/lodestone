import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
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

  // PlayitggOverview: the Playit integration page
  'console.playit.loading': 'playit.gg 載入中',
  'console.playit.loadFailed': '無法載入 playit.gg 狀態：{error}',
  'console.playit.signupFailed': '無法產生註冊連結：{error}',
  'console.playit.verifyFailed': '無法驗證 playit.gg 帳戶：{error}',
  'console.playit.startFailed': '無法啟動 playit.gg 執行程式：{error}',
  'console.playit.stopFailed': '無法停止 playit.gg 執行程式：{error}',
  'console.playit.setupTitle': '設定 Playitgg',
  'console.playit.setupIntro':
    '你需要先註冊 Playitgg 才能使用此功能。使用下方的按鈕產生註冊連結！',
  'console.playit.generateLink': '產生連結',
  'console.playit.setupFollowLink':
    '開啟下方的連結並建立帳戶！建立帳戶並進入控制面板後，按下方的「驗證」。',
  'console.playit.verify': '驗證',
  'console.playit.runnerTitle': 'Playitgg 執行程式',
  'console.playit.runner.Stopped': '已停止',
  'console.playit.runner.Loading': '載入中',
  'console.playit.runner.Started': '已啟動',
  'console.playit.trouble':
    '如果遇到問題，請確認已在網站上正確設定通道，並且{running}！',
  'console.playit.troubleRunning': '你的伺服器確實正在執行',
  'console.playit.signupRequired': '你需要先註冊 Playitgg 才能使用此功能。',
  'console.playit.signupLink': '按這裡註冊。',
  'console.playit.tunnels': '通道',
  'console.playit.refreshTunnels': '重新整理通道',
  'console.playit.tunnelsIntro':
    '你的通道列在下方。你可以在 {link} 網站的控制面板中建立更多通道並變更設定。',
  'console.playit.loadingTunnels': '通道載入中',
  'console.playit.localServer': '本機伺服器位址：{address}',
  'console.playit.copyAddress': '複製位址',
  'console.playit.noTunnels': '你還沒有建立任何通道',
  'console.playit.notSignedUp': '你還沒有註冊',
  'console.playit.help': '需要協助？',
} satisfies CatalogOf<typeof en>;
