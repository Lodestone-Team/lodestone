import { Radio, RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import { ThemePreference, useThemePreference } from 'utils/theme';

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
    <div className="flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-medium">
      <div className="flex min-w-0 grow flex-col">
        <label
          id="theme-label"
          className="text-medium font-medium text-gray-300"
        >
          Theme
        </label>
        <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
          {descriptions[preference]}
        </div>
      </div>
      <RadioGroup
        value={preference}
        onChange={setPreference}
        aria-labelledby="theme-label"
        className="flex w-5/12 shrink-0 flex-row child:grow"
      >
        {options.map(({ value, label }) => (
          <Radio
            key={value}
            value={value}
            className={clsx(
              'input-background input-outlines input-text-style input-border-normal',
              'cursor-pointer overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md',
              'focus-visible:ring-blue-faded/50'
            )}
          >
            {({ checked }) => (
              <span
                className={clsx(
                  'block h-full w-full py-1.5 px-3 text-center select-none',
                  checked
                    ? 'bg-blue-faded/30 text-gray-300'
                    : 'text-white/75 hover:bg-gray-700'
                )}
              >
                {label}
              </span>
            )}
          </Radio>
        ))}
      </RadioGroup>
    </div>
  );
}
