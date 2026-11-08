import { useEffect, useId, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString, cn } from 'utils/util';
import { Toggle } from '../Toggle';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { SettingsRow } from 'components/Settings/SettingsBlocks';
import { useT } from 'i18n';

/**
 * A self controlled toggle component meant to represent a single value of a config
 *
 * It is NOT meant to be used as a form input
 *
 * See ToggleField for that
 */
export default function ToggleBox({
  label,
  value: initialValue,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
  canRead = true,
  isLoading: isLoadingProp = false,
  description,
  descriptionFunc,
  optimistic = true, // if true, the toggle will change immediately and go into loading state, and will change back if onChange throws an error
}: {
  label: string;
  value: boolean;
  className?: string;
  error?: string;
  disabled?: boolean;
  canRead?: boolean;
  isLoading?: boolean;
  onChange: (arg: boolean) => Promise<void>;
  description?: React.ReactNode;
  descriptionFunc?: (arg: boolean) => React.ReactNode;
  optimistic?: boolean;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const labelId = useId();
  const t = useT();

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  const onChange = async (newValue: boolean) => {
    if (optimistic) setValue(newValue);
    setIsLoading(true);
    const submitError = await catchAsyncToString(onChangeProp(newValue));
    setError(submitError);
    setIsLoading(false);
    if (submitError.length > 0) {
      setValue(initialValue);
    }
  };

  const errorText = errorProp || error;
  disabled = disabled || !canRead || isLoadingProp;
  description = canRead
    ? (descriptionFunc?.(initialValue || value) ?? description)
    : t('common.noPermission');

  return (
    <SettingsRow
      className={cn('group relative', className)}
      label={label}
      labelId={labelId}
      hint={description}
      error={errorText}
    >
      {isLoadingProp ? (
        // the value is still loading: the switch's shape (after a moment)
        <SkeletonGroup className="flex flex-row justify-end">
          <Skeleton className="h-6 w-11 rounded-full" />
        </SkeletonGroup>
      ) : (
        <>
          {isLoading && (
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
          )}
          <Toggle
            value={value}
            onChange={onChange}
            disabled={disabled || isLoading}
            aria-labelledby={labelId}
          />
        </>
      )}
    </SettingsRow>
  );
}
