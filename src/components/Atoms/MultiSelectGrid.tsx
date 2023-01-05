import clsx from 'clsx';
import Checkbox from './Checkbox';

export type MultiSelectGridProps<T extends string | object> = {
  className?: string;
  disabled?: boolean;
  options: T[];
  isLoading?: boolean;
  selectedOptions: T[];
  onChange: (selectedOptions: T[]) => void;
  optionLabel?: (option: T) => string;
};

/**
 * A grid of checkboxes meant to be used as a controlled component
 */
export default function MultiSelectGrid<T extends string | object>(
  props: MultiSelectGridProps<T>
) {
  const {
    className,
    disabled,
    options,
    selectedOptions,
    onChange,
    optionLabel = (option) => {
      let output = '';
      if (typeof option === 'string') {
        output = option;
      } else {
        output = JSON.stringify(option);
      }
      console.log('optionLabel', option, output);
      return output;
    },
  } = props;

  const onCheckboxChange = (option: T, checked: boolean) => {
    if (checked) {
      onChange([...selectedOptions, option]);
    } else {
      onChange(selectedOptions.filter((o) => o !== option));
    }
  };

  return (
    <div
      className={clsx(
        'grid grid-cols-2 gap-4 @lg:grid-cols-4',
        disabled ? 'bg-gray-850' : 'bg-gray-800',
        className
      )}
    >
      {options.map((option) => (
        <Checkbox
          key={optionLabel(option)}
          label={optionLabel(option)}
          checked={selectedOptions.includes(option)}
          onChange={(checked) => onCheckboxChange(option, checked)}
          disabled={disabled}
          className="pr-4"
        />
      ))}
    </div>
  );
}
