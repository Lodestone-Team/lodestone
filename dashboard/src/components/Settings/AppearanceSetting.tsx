import { Radio, RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import { MessageKey, useT } from 'i18n';
import { ThemePreference, useThemePreference } from 'utils/theme';
import { SettingsRow } from './SettingsBlocks';

const options = [
  { value: 'system', labelKey: 'settings.theme.system' },
  { value: 'light', labelKey: 'settings.theme.light' },
  { value: 'dark', labelKey: 'settings.theme.dark' },
] as const satisfies readonly {
  value: ThemePreference;
  labelKey: MessageKey;
}[];

const descriptions = {
  system: 'settings.theme.systemHint',
  light: 'settings.theme.lightHint',
  dark: 'settings.theme.darkHint',
} as const satisfies Record<ThemePreference, MessageKey>;

/**
 * The light / dark theme choice. It is saved in this browser (localStorage),
 * not on the core, so each browser and Lodestone Host keep their own.
 */
export default function AppearanceSetting() {
  const t = useT();
  const [preference, setPreference] = useThemePreference();

  return (
    <SettingsRow
      label={t('settings.theme.label')}
      labelId="theme-label"
      hint={t(descriptions[preference])}
    >
      {/* a segmented control */}
      <RadioGroup
        value={preference}
        onChange={setPreference}
        aria-labelledby="theme-label"
        className="flex flex-row rounded-lg border border-gray-700 bg-gray-900 p-0.5 light:border-line-input"
      >
        {options.map(({ value, labelKey }) => (
          <Radio
            key={value}
            value={value}
            className={clsx(
              'cursor-pointer rounded-md px-2.5 py-1 whitespace-nowrap select-none',
              'text-[0.8125rem] font-mediumbold',
              'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
              'text-gray-400 hover:text-gray-300',
              'data-checked:bg-gray-700 data-checked:text-gray-300',
              // light: the chosen one is a raised white segment
              'light:data-checked:bg-gray-800 light:data-checked:shadow-sm'
            )}
          >
            {t(labelKey)}
          </Radio>
        ))}
      </RadioGroup>
    </SettingsRow>
  );
}
