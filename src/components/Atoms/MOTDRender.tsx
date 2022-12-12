import clsx from 'clsx';
import { ParseResult } from 'minecraft-motd-util/dist/types';

export const MOTDRender = ({
  motd,
  className,
}: {
  motd: ParseResult;
  className?: string;
}) => {
  console.log('MOTDRender', motd)
  return (
    <>
      {motd.map((item, index) =>
        item.text.split('\n').map((text, index) => (
          <>
            {index > 0 && <br />}
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
              {text}
            </span>
          </>
        ))
      )}
    </>
  );
};
