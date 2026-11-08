import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  'settings.language.label': 'Langue',
  'settings.language.hint':
    'La langue du tableau de bord. Les noms, journaux et messages des serveurs et du core restent tels quels.',
  'settings.language.auto': 'Langue du navigateur ({language})',
} satisfies CatalogOf<typeof en>;
