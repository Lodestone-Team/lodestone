import { useEffect, useState, Fragment, useMemo, useRef } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { Combobox, Transition } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import {
  faAsterisk,
  faSort,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';
import { useVirtualizer, VirtualItem } from '@tanstack/react-virtual';
import { VirtualizedList } from '../../../Atoms/Form/ComboField';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { useT } from 'i18n';

/**
 * A combo box field that allows the user to select from a list of options, or
 * enter a custom value. Meant to be used with Formik.
 *
 * Slightly buggy since we are using react-virtual to render the options, this
 * should only be used where there are a large number of options.
 */
export type ComboFieldProps = FieldHookConfig<string> & {
  label?: string;
  placeholder?: string;
  loading?: boolean;
  options: string[];
  allowCustom?: boolean;
  description?: string;
  optional?: boolean;
  filterOptions?: (query: string, options: string[]) => string[];
};

const defaultFilterOptions = (query: string, options: string[]) => {
  return query === ''
    ? options
    : options.filter((option) => {
        return option.toLowerCase().includes(query.toLowerCase());
      });
};

export default function FormComboField(props: ComboFieldProps) {
  const {
    label,
    className,
    disabled,
    options,
    placeholder,
    loading,
    allowCustom,
    description,
    optional,
    filterOptions = defaultFilterOptions,
    ...rest
  } = props;
  const t = useT();
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [query, setQuery] = useState('');
  const isError = meta.touched && meta.error && true;
  const errorText = isError ? meta.error : '';
  const disabledVisual = disabled || loading;
  const loadingVisual = loading && !disabled;

  // reset the field value if the options change
  useEffect(() => {
    if (selectedValue && !options.includes(selectedValue) && !allowCustom) {
      field.onChange({
        target: {
          name: field.name,
          value: '',
        },
      });
      setQuery('');
      console.log('resetting field value');
    }
  }, [options, selectedValue]);

  const filteredOptions = useMemo(
    () => filterOptions(query, options),
    [query, options]
  );

  const icon = loadingVisual ? (
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
    <>
      <div
        className={clsx(
          'group relative flex flex-row items-center justify-between',
          'gap-4 bg-gray-800 px-4 py-3 text-medium',
          className
        )}
      >
        <div className={`flex min-w-0 grow flex-col`}>
          {label && (
            <label className="relative text-medium font-medium text-gray-300">
              {label}
              {!optional && (
                <span className="absolute mt-1 ml-1 text-[8px] text-red-200">
                  <FontAwesomeIcon icon={faAsterisk} />
                </span>
              )}
            </label>
          )}
          {errorText ? (
            <div className="text-medium font-medium tracking-medium text-red-200">
              {errorText || t('common.unknownError')}
            </div>
          ) : (
            <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50">
              {description}
            </div>
          )}
        </div>
        {loadingVisual && (
          // the options are loading: the field's shape (after a moment)
          <SkeletonGroup className="w-5/12 shrink-0">
            <Skeleton className="h-8.5 w-full rounded" />
          </SkeletonGroup>
        )}
        <div
          className={clsx(
            'relative w-5/12 shrink-0',
            loadingVisual && 'hidden'
          )}
        >
          <Combobox
            value={selectedValue ? selectedValue : ''}
            name={field.name}
            // open the options when the input is focused
            immediate
            onChange={(newValue: string | null) => {
              // Headless UI 2 comboboxes are nullable and emit null when the input
              // is cleared; keep the previous value like the non-nullable v1 one
              if (newValue === null) return;
              // need to generate a fake React.ChangeEvent
              const event = {
                target: {
                  name: field.name,
                  value: newValue,
                },
              };
              field.onChange(event);
            }}
            disabled={disabledVisual}
          >
            <Combobox.Input
              className={clsx(
                'input-outlines input-text-style group min-h-[1em] w-full rounded px-3 py-1.5',
                'enabled:hover:outline-white/30 enabled:ui-not-open:hover:bg-gray-700',
                'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850',
                'enabled:ui-not-open:bg-gray-850 enabled:ui-not-open:active:bg-gray-850',
                'enabled:ui-not-open:active:outline-white/30',
                errorText ? 'input-border-error' : 'input-border-normal',
                selectedValue ? 'text-gray-300' : 'text-gray-500'
              )}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={placeholder}
            />
            <Combobox.Button className="group absolute inset-y-0 right-0 flex items-center pr-1.5">
              {icon}
            </Combobox.Button>
            <Transition
              as={Fragment}
              leave="transition ease-in duration-150"
              leaveFrom="opacity-100 translate-y-0"
              leaveTo="opacity-0 -translate-y-1"
              afterLeave={() => setQuery('')}
            >
              <Combobox.Options
                className={clsx('absolute z-40 mt-2 w-full pb-4')}
              >
                <div
                  className={clsx(
                    'input-shape input-outlines input-text-style w-full rounded-md',
                    'bg-gray-850 p-0 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
                  )}
                >
                  {filteredOptions.length === 0 ? (
                    allowCustom ? (
                      <Combobox.Option
                        value={query}
                        className={clsx(
                          'relative cursor-default rounded-md py-2 pr-4 pl-3 text-gray-300 select-none',
                          'border-t border-gray-faded/30 last:border-b ui-active:z-50 ui-active:mb-[-1px] ui-active:border-y ui-active:border-white/50 ui-active:last:mb-0',
                          'ui-selected:font-medium ui-not-selected:font-medium',
                          'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                          'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
                        )}
                      >
                        <div className="flex flex-row justify-between">
                          <span className="block truncate pr-1">
                            {t('create.combo.add', { query })}
                          </span>
                        </div>
                      </Combobox.Option>
                    ) : (
                      <div className="relative cursor-default rounded-md bg-gray-800 py-2 pr-4 pl-3 text-gray-300 select-none">
                        {t('create.combo.nothingFound')}
                      </div>
                    )
                  ) : (
                    <VirtualizedList
                      items={filteredOptions}
                      selectedValue={selectedValue}
                    />
                  )}
                </div>
              </Combobox.Options>
            </Transition>
          </Combobox>
        </div>
      </div>
    </>
  );
}
