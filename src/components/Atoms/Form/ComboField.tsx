import React, { useEffect, useState, Fragment, useMemo, useRef } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { Combobox, Transition } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';
import { useVirtualizer, VirtualItem } from '@tanstack/react-virtual';

/**
 * A combo box field that allows the user to select from a list of options, or
 * enter a custom value. Meant to be used with Formik.
 * 
 * Slightly buggy since we are using react-virtual to render the options, this 
 * should only be used where there are a large number of options.
 */
export type ComboFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
  allowCustom?: boolean;
  filterOptions?: (query: string, options: string[]) => string[];
};

const defaultFilterOptions = (query: string, options: string[]) => {
  return query === ''
    ? options
    : options.filter((option) => {
        return option.toLowerCase().includes(query.toLowerCase());
      });
};

export default function ComboField(props: ComboFieldProps) {
  const {
    label,
    className,
    disabled,
    options,
    placeholder,
    loading,
    allowCustom,
    filterOptions = defaultFilterOptions,
    ...rest
  } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [query, setQuery] = useState('');
  const buttonRef = React.useRef<HTMLButtonElement>(null);
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
    <div
      className={`flex flex-col gap-1 ${className} group relative text-base`}
    >
      <label className="absolute -top-6 text-small font-medium text-gray-300">
        {label ? `${label}:` : ''}
      </label>
      <div className="relative mt-1">
        <Combobox
          value={selectedValue ? selectedValue : ''}
          name={field.name}
          onChange={(newValue: string) => {
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
          {({ activeIndex }) => (
            <>
              <Combobox.Input
                className={clsx(
                  'input-base group min-h-[1em] w-full py-1.5 px-3',
                  'enabled:hover:outline-white/30 enabled:ui-not-open:hover:bg-gray-700',
                  'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850 ',
                  'enabled:ui-not-open:bg-gray-850  enabled:ui-not-open:active:bg-gray-850',
                  'enabled:ui-not-open:active:outline-white/30',
                  errorText ? 'border-error' : 'border-normal',
                  selectedValue ? 'text-gray-300' : 'text-gray-500'
                )}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={placeholder}
                onClick={() => {
                  buttonRef.current?.click();
                }}
              />
              <Combobox.Button
                ref={buttonRef}
                className="group absolute inset-y-0 right-0 flex items-center pr-1.5"
              >
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
                  className={clsx(
                    'input-base absolute z-40 mt-2 w-full rounded-md',
                    'bg-gray-850 p-0 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
                  )}
                >
                  {filteredOptions.length === 0 ? (
                    allowCustom ? (
                      <Combobox.Option
                        value={query}
                        className={clsx(
                          'relative cursor-default select-none rounded-md py-2 pl-3 pr-4 text-gray-300',
                          'border-t border-gray-faded/30 last:border-b ui-active:z-50 ui-active:mb-[-1px] ui-active:border-y ui-active:border-white/50 ui-active:last:mb-0',
                          'ui-selected:font-medium ui-not-selected:font-normal',
                          'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                          'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
                        )}
                      >
                        <div className="flex flex-row justify-between">
                          <span className="block truncate pr-1">
                            Add &#34;{query}&#34;
                          </span>
                        </div>
                      </Combobox.Option>
                    ) : (
                      <div className="relative cursor-default select-none rounded-md bg-gray-800 py-2 pl-3 pr-4 text-gray-300">
                        Nothing found.
                      </div>
                    )
                  ) : (
                    <VirtualizedList
                      items={filteredOptions}
                      selectedValue={selectedValue}
                    />
                  )}
                </Combobox.Options>
              </Transition>
            </>
          )}
        </Combobox>
        {errorText && (
          <div
            className={`absolute -bottom-6 whitespace-nowrap text-right font-sans text-small not-italic text-red
          `}
          >
            {errorText || 'Unknown error'}
          </div>
        )}
      </div>
    </div>
  );
}

function VirtualizedList({
  items,
  selectedValue,
}: {
  items: string[];
  selectedValue: string | null;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const selectedIndex = useMemo(
    () => items.findIndex((item) => item === selectedValue),
    [selectedValue]
  );

  const rowVirtualizer = useVirtualizer({
    count: items?.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 35,
    overscan: 5,
    paddingEnd: 8,
    paddingStart: 8,
    getItemKey: (index) => items[index],
    initialOffset:
      selectedIndex && selectedIndex > 0 ? selectedIndex * 35 - 20 : 0,
  });

  return (
    <div
      ref={parentRef}
      className="overflow-y-overlay max-h-60 w-full overflow-auto rounded-md"
    >
      <div
        className="relative z-40 w-full"
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow: VirtualItem) => (
          <Combobox.Option
            key={virtualRow.index}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: `${virtualRow.size}px`,
              transform: `translateY(${virtualRow.start}px)`,
            }}
            className={clsx(
              'relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300',
              'border-t border-gray-faded/30 last:border-b ui-active:z-50 ui-active:border-y ui-active:border-white/50 ui-active:last:mb-0',
              'ui-selected:font-medium ui-not-selected:font-normal',
              'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
              'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
            )}
            value={items?.[virtualRow.index]}
          >
            {({ selected, active }) => (
              <div className="flex flex-row justify-between">
                <span className="block truncate pr-1">
                  {items?.[virtualRow.index]}
                </span>
              </div>
            )}
          </Combobox.Option>
        ))}
      </div>
    </div>
  );
}
