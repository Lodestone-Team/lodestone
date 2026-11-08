import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': '語言',
  'settings.language.hint':
    '控制面板的顯示語言。來自伺服器和核心的名稱、日誌和訊息會維持原樣。',
  'settings.language.auto': '跟隨瀏覽器（{language}）',
} satisfies CatalogOf<typeof en>;
