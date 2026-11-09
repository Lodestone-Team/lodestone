//! Traditional Chinese, `zh-Hant` (Taiwan usage).
//!
//! Glossary: use these words everywhere, so a thing has one name.
//!
//! | English | 繁體中文 |
//! |---|---|
//! | instance (a launcher game folder) | 實例 |
//! | server (a game server a core runs) | 伺服器 |
//! | core (a Lodestone Core) | 核心；Lodestone Core 保留原文 |
//! | mod / modpack | 模組 / 模組包 |
//! | mod loader / loader | 模組載入器 / 載入器 |
//! | resource pack / shader pack | 資源包 / 光影包 |
//! | world / backup / screenshot | 世界 / 備份 / 截圖 |
//! | Settings (the dialog and the tab) | 設定 |
//! | Instance defaults | 實例預設設定 |
//! | shared game settings | 共用遊戲設定 |
//! | account / offline account | 帳號 / 離線帳號 |
//! | sign in / sign out | 登入 / 登出 |
//! | Play | 開始遊戲 |
//! | Save (a change) / Discard | 儲存 / 捨棄 |
//! | download / install / update | 下載 / 安裝 / 更新 |
//! | game version | 遊戲版本 |
//! | memory | 記憶體 |
//! | console / log | 主控台 / 日誌 |
//! | key binding | 按鍵 |
//! | player / operator / whitelist | 玩家 / 管理員 / 白名單 |
//! | folder / file | 資料夾 / 檔案 |
//! | username / password | 使用者名稱 / 密碼 |
//! | user / owner / permission | 使用者 / 擁有者 / 權限 |
//! | API key / token | API 金鑰 / 權杖 |
//! | Undo / Redo | 復原 / 重做 |
//! | restore (a backup, from the Trash) | 還原 |
//! | build (a release build) | 組建 |
//! | dashboard | 控制面板 |
//! | profile (Minecraft / Xbox) | 個人檔案 |
//! | narrator / FOV (Minecraft's own words) | 朗讀功能 / 視角廣度 |
//! | beta / alpha (Modrinth versions) | 測試版 / 早期測試版 |
//! | start / stop / restart / kill | 啟動 / 停止 / 重新啟動 / 強制停止 |
//! | running / stopped | 執行中 / 已停止 |
//! | port / address / connection | 連接埠 / 位址 / 連線 |
//! | enable / disable | 啟用 / 停用 |
//! | the Trash | 垃圾桶（Windows：資源回收筒） |
//! | Keychain | 鑰匙圈 |
//!
//! The words match the Lodestone dashboard's (`dashboard/src/i18n/zh-Hant.ts`); call people「你」.
//!
//! Style: full-width punctuation (，。：；？！（）「」), a space between Chinese and
//! Latin letters or digits ("4 GB 記憶體", "Fabric 實例"), no plural forms. Product
//! names stay in English.

pub mod common;
pub mod format;
