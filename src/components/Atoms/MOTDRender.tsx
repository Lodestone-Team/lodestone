import clsx from 'clsx';
import { ParseResult } from 'minecraft-motd-util/dist/types';
import React from 'react';

const colors: {
  [key: string]: string;
} = {
  black: '#000000',
  dark_blue: '#0000AA',
  dark_green: '#00AA00',
  dark_aqua: '#00AAAA',
  dark_red: '#AA0000',
  dark_purple: '#AA00AA',
  gold: '#FFAA00',
  gray: '#AAAAAA',
  dark_gray: '#555555',
  blue: '#5555FF',
  green: '#55FF55',
  aqua: '#55FFFF',
  red: '#FF5555',
  light_purple: '#FF55FF',
  yellow: '#FFFF55',
  white: '#FFFFFF',
  minecoin_gold: '#DDD605',
};

export const MOTDRender = ({
  motd,
  className,
}: {
  motd: ParseResult;
  className?: string;
}) => {
  let lineNum = 0;
  return (
    <>
      {motd.map((item, index) =>
        item.text.split('\n').map((text, brindex) => {
          if (text === '') return null;
          if (brindex > 0) lineNum++;
          // only display 2 lines
          if (lineNum > 1) return null;
          return (
            <React.Fragment key={index}>
              {brindex > 0 && <br />}
              <span
                style={{
                  color: item.color in colors ? colors[item.color] : undefined,
                }}
                className={clsx(
                  item.bold ? 'font-bold' : 'font-normal',
                  item.italics ? 'italic' : '',
                  item.underline ? 'underline' : '',
                  item.strikethrough ? 'line-through' : ''
                )}
              >
                {text}
              </span>
            </React.Fragment>
          );
        })
      )}
      {lineNum > 1 && <span>...</span>}
    </>
  );
};
