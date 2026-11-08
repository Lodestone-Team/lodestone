import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': '语言',
  'settings.language.hint':
    '控制面板的显示语言。来自服务器和核心的名称、日志和消息将保持原样。',
  'settings.language.auto': '跟随浏览器（{language}）',
} satisfies CatalogOf<typeof en>;
