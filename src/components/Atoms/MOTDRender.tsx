import clsx from 'clsx';
import { ParseResult } from 'minecraft-motd-util/dist/types';

export const MOTDRender = ({
  motd,
  className,
}: {
  motd: ParseResult;
  className?: string;
}) => {
  return (
    <>
      {motd.map((item, index) => {
        return (
          <span
            style={{
              color: item.color,
            }}
            className={clsx(
              item.bold ? 'font-bold' : 'font-normal',
              item.italics ? 'italic' : '',
              item.underline ? 'underline' : '',
              item.strikethrough ? 'line-through' : ''
            )}
            key={index}
          >
            {item.text}
          </span>
        );
      })}
    </>
  );
};
