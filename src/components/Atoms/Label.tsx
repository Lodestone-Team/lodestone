import clsx from 'clsx';

export type LabelSize = 'small' | 'medium' | 'large';

export type LabelColor = 'green' | 'yellow' | 'red' | 'gray';

export default function Label({
  size = 'small',
  color = 'gray',
  className = '',
  children,
  onClick,
  ...rest
}: {
  size: LabelSize;
  color: LabelColor;
  className?: string;
  children: React.ReactNode;
  onClick?: (e: React.MouseEvent<HTMLSpanElement>) => void;
}) {
  return (
    <span
      className={clsx(
        {
          small: 'py-[0.125em] px-2 text-small font-medium tracking-medium',
          medium: 'py-1 px-2 text-small font-medium tracking-medium',
          large: 'py-1 px-2 text-medium font-medium tracking-medium',
        }[size],
        {
          green: 'bg-green-faded/25 text-green',
          yellow: 'bg-yellow-faded/25 text-yellow',
          red: 'bg-red-faded/25 text-red',
          gray: 'bg-gray-faded/30 text-gray-300',
        }[color],
        `h-fit select-none whitespace-nowrap rounded-full font-bold tracking-medium ${className}`
      )}
      onClick={onClick}
      {...rest}
    >
      {children}
    </span>
  );
}
