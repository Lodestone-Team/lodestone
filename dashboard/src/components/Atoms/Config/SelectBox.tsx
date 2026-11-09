import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox, Transition } from '@headlessui/react';
import clsx from 'clsx';
import { useEffect, useId, useState, Fragment } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { SettingsRow } from 'components/Settings/SettingsBlocks';
import { useT } from 'i18n';

/**
 * A self controlled dropdown meant to represent a single value of a config
 *
 * It is NOT meant to be used as a form input
 *
 * See SelectField for that
 */
export default function SelectBox({
  label,
  value: initialValue,
  options,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
  canRead = true,
  isLoading: isLoadingProp = false,
  description,
  descriptionFunc,
  actionIcon,
  actionIconClick,
  optimistic = true, // if true, the dropdown will change immediately and go into loading state, and will change back if onChange throws an error
  optionLabel = (option) => option,
}: {
  label: string;
  value?: string;
  options: string[];
  className?: string;
  error?: string;
  disabled?: boolean;
  canRead?: boolean;
  isLoading?: boolean;
  onChange: (arg: string) => Promise<void>;
  description?: React.ReactNode;
  descriptionFunc?: (arg: string) => React.ReactNode;
  actionIcon?: IconDefinition;
  actionIconClick?: () => void;
  optimistic?: boolean;
  /** the text shown for an option, e.g. a translated name (default: itself) */
  optionLabel?: (option: string) => string;
}) {
  const t = useT();
  // '' while nothing is chosen: the button shows "Select…"
  const [value, setValue] = useState(initialValue || '');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const labelId = useId();

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue || '');
  }, [initialValue]);

  const onChange = async (newValue: string) => {
    if (optimistic) setValue(newValue);
    setIsLoading(true);
    const submitError = await catchAsyncToString(onChangeProp(newValue));
    setError(submitError);
    setIsLoading(false);
    if (submitError.length > 0) {
      setValue(initialValue || '');
    }
  };

  const errorText = errorProp || error;
  disabled = disabled || !canRead || isLoadingProp;
  description = canRead
    ? (descriptionFunc?.(initialValue || value) ?? description)
    : t('common.noPermission');

  const icon = isLoading ? (
    <BeatLoader
      key="loading"
      size="0.25rem"
      cssOverride={{
        width: '2rem',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        margin: `0 -0.5rem`,
      }}
      color="#6b7280"
    />
  ) : (
    <FontAwesomeIcon
      key="icon"
      icon={faSort}
      className={clsx(
        'w-4 text-gray-faded/30',
        'group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500'
      )}
    />
  );

  return (
    <SettingsRow
      className={clsx('group relative', className)}
      label={label}
      labelId={labelId}
      hint={description}
      error={errorText}
      controlClassName="w-5/12"
    >
      {isLoadingProp && (
        // the value is still loading: the dropdown's shape (after a moment)
        <SkeletonGroup className="w-full">
          <Skeleton className="h-8 w-full rounded" />
        </SkeletonGroup>
      )}
      <div className={clsx('relative w-full', isLoadingProp && 'hidden')}>
        <Listbox
          value={value}
          onChange={onChange}
          disabled={disabled || isLoading}
        >
          <div className="relative w-full">
            <Listbox.Button
              aria-labelledby={labelId}
              className={clsx(
                'input-outlines input-text-style group min-h-[1em] w-full rounded px-3 py-1.5',
                'enabled:hover:outline-white/30',
                // gray-900 like the text fields, so it shows on the gray-850 cards
                'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-900',
                'enabled:ui-not-open:bg-gray-900 enabled:ui-not-open:hover:bg-gray-700',
                'enabled:ui-not-open:active:bg-gray-900 enabled:ui-not-open:active:outline-white/30',
                errorText ? 'input-border-error' : 'input-border-normal'
              )}
            >
              {value
                ? options.includes(value)
                  ? optionLabel(value)
                  : value
                : t('common.select')}
              <div
                className={clsx(
                  'pointer-events-none absolute items-center justify-end',
                  'top-0 right-0 flex h-full flex-row px-3 py-1.5'
                )}
              >
                <div className="flex flex-row gap-2">{icon}</div>
              </div>
            </Listbox.Button>

            <Transition
              as={Fragment}
              leave="transition ease-in duration-150"
              leaveFrom="opacity-100 translate-y-0"
              leaveTo="opacity-0 -translate-y-1"
            >
              {/* anchored below the button in a portal, as wide as the button */}
              <Listbox.Options
                anchor={{ to: 'bottom', gap: 8 }}
                className={clsx(
                  'input-outlines input-text-style z-40 max-h-60 w-(--button-width) overflow-auto rounded p-0 py-1',
                  'bg-gray-850 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
                )}
              >
                {options.map((option) => (
                  <Listbox.Option
                    key={option}
                    value={option}
                    className={clsx(
                      'relative cursor-pointer py-2 pr-4 pl-3 text-gray-300 select-none',
                      'border-t border-gray-faded/30 last:border-b ui-active:z-50 ui-active:mb-[-1px] ui-active:border-y ui-active:border-white/50 ui-active:last:mb-0',
                      'ui-selected:font-medium ui-not-selected:font-medium',
                      'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                      'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
                    )}
                  >
                    {({ active }) => (
                      <div className="flex flex-row justify-between">
                        <span className="block truncate pr-1">
                          {optionLabel(option)}
                        </span>
                        <div
                          onClick={actionIconClick}
                          className="absolute right-3"
                        >
                          {active && actionIcon && actionIconClick && (
                            <FontAwesomeIcon
                              key="icon"
                              icon={actionIcon}
                              className="w-4 cursor-pointer text-gray-faded/30 hover:text-gray-500"
                            />
                          )}
                        </div>
                      </div>
                    )}
                  </Listbox.Option>
                ))}
                {(initialValue === undefined || initialValue.length === 0) && (
                  <Listbox.Option
                    key="select-placeholder"
                    value={''}
                    className={`relative cursor-default bg-gray-700 py-2 pr-4 pl-8 text-gray-400 select-none`}
                    disabled
                  >
                    <span className={`block truncate font-medium`}>
                      {t('common.select')}
                    </span>
                  </Listbox.Option>
                )}
              </Listbox.Options>
            </Transition>
          </div>
        </Listbox>
      </div>
    </SettingsRow>
  );
}
