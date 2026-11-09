//! Simplified Chinese, `zh-Hans` (mainland China).
//!
//! Glossary: use these words everywhere, so a thing has one name.
//!
//! | English | 简体中文 |
//! |---|---|
//! | instance (a launcher game folder) | 实例 |
//! | server (a game server a core runs) | 服务器 |
//! | core (a Lodestone Core) | 核心；Lodestone Core 保留原文 |
//! | mod / modpack | 模组 / 整合包 |
//! | mod loader / loader | 模组加载器 / 加载器 |
//! | resource pack / shader pack | 资源包 / 光影包 |
//! | world / backup / screenshot | 世界 / 备份 / 截图 |
//! | Settings (the dialog and the tab) | 设置 |
//! | Instance defaults | 实例默认设置 |
//! | shared game settings | 共享游戏设置 |
//! | account / offline account | 账户 / 离线账户 |
//! | sign in / sign out | 登录 / 退出登录 |
//! | Play | 开始游戏 |
//! | Save (a change) / Discard | 保存 / 放弃 |
//! | download / install / update | 下载 / 安装 / 更新 |
//! | game version | 游戏版本 |
//! | memory | 内存 |
//! | console / log | 控制台 / 日志 |
//! | key binding | 按键 |
//! | player / operator / whitelist | 玩家 / 管理员 / 白名单 |
//! | folder / file | 文件夹 / 文件 |
//! | username / password | 用户名 / 密码 |
//! | user / owner / permission | 用户 / 所有者 / 权限 |
//! | API key / token | API 密钥 / 令牌 |
//! | narrator / FOV (Minecraft's own words) | 复述功能 / 视场角 |
//! | Minecraft's natives | 本地库 |
//! | beta / alpha (Modrinth versions) | 测试版 / 早期测试版 |
//! | start / stop / restart / kill | 启动 / 停止 / 重启 / 强制停止 |
//! | running / stopped | 运行中 / 已停止 |
//! | port / address / connection | 端口 / 地址 / 连接 |
//! | enable / disable | 启用 / 禁用 |
//! | the Trash | 废纸篓（Windows：回收站） |
//! | Keychain | 钥匙串 |
//!
//! The words match the Lodestone dashboard's (`dashboard/src/i18n/zh-Hans.ts`); call people “你”.
//!
//! Style: full-width punctuation (，。：；？！（）“”), a space between Chinese and
//! Latin letters or digits ("4 GB 内存", "Fabric 实例"), no plural forms. Product names
//! stay in English.

pub mod content;
pub mod instance_settings;
pub mod instances;
pub mod servers;
pub mod shared_options;
pub mod shell;
pub mod worlds;
