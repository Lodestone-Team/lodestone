import { Radio, RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import { ThemePreference, useThemePreference } from 'utils/theme';
import { SettingsRow } from './SettingsBlocks';

const options: { value: ThemePreference; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

const descriptions: Record<ThemePreference, string> = {
  system: "Follows your device's light or dark setting",
  light: 'Always light',
  dark: 'Always dark',
};

/**
 * The light / dark theme choice. It is saved in this browser (localStorage),
 * not on the core, so each browser and the desktop app keep their own.
 */
export default function AppearanceSetting() {
  const [preference, setPreference] = useThemePreference();

  return (
    <SettingsRow
      label="Theme"
      labelId="theme-label"
      hint={descriptions[preference]}
    >
      {/* a segmented control */}
      <RadioGroup
        value={preference}
        onChange={setPreference}
        aria-labelledby="theme-label"
        className="flex flex-row rounded-lg border border-gray-700 bg-gray-900 p-0.5 light:border-line-input"
      >
        {options.map(({ value, label }) => (
          <Radio
            key={value}
            value={value}
            className={clsx(
              'cursor-pointer rounded-md px-2.5 py-1 select-none',
              'text-[0.8125rem] font-mediumbold',
              'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
              'text-gray-400 hover:text-gray-300',
              'data-checked:bg-gray-700 data-checked:text-gray-300',
              // light: the chosen one is a raised white segment
              'light:data-checked:bg-gray-800 light:data-checked:shadow-sm'
            )}
          >
            {label}
          </Radio>
        ))}
      </RadioGroup>
    </SettingsRow>
  );
}
