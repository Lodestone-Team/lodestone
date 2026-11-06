/**
 * Minecraft MOTD parser.
 *
 * Vendored from minecraft-motd-util 1.1.12 (src/parse.ts and src/types.ts),
 * https://github.com/PassTheMayo/minecraft-motd-util, which is no longer
 * maintained. Lodestone change: the default text color is 'gray' instead of
 * 'white' (previously applied through patch-package).
 *
 * MIT License
 *
 * Copyright (c) 2021 Jacob Gunther
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

export type FormattingProperties =
  | 'bold'
  | 'italics'
  | 'underline'
  | 'strikethrough'
  | 'obfuscated';

export interface ParseItem {
  color: string;
  bold?: boolean;
  italics?: boolean;
  underline?: boolean;
  strikethrough?: boolean;
  obfuscated?: boolean;
  text: string;
}

export type ParseResult = ParseItem[];

export interface ParseOptions {
  formattingCharacter?: string;
}

export interface Chat {
  text: string;
  translate?: string;
  color?: string;
  bold?: string;
  italic?: string;
  underlined?: string;
  strikethrough?: string;
  obfuscated?: string;
  extra?: Chat[];
}

const DEFAULT_COLOR = 'gray';

const colorLookupNames: Record<string, string> = {
  '0': 'black',
  '1': 'dark_blue',
  '2': 'dark_green',
  '3': 'dark_aqua',
  '4': 'dark_red',
  '5': 'dark_purple',
  '6': 'gold',
  '7': 'gray',
  '8': 'dark_gray',
  '9': 'blue',
  a: 'green',
  b: 'aqua',
  c: 'red',
  d: 'light_purple',
  e: 'yellow',
  f: 'white',
  g: 'minecoin_gold',
};

const formattingLookupProperties: Record<string, FormattingProperties> = {
  k: 'obfuscated',
  l: 'bold',
  m: 'strikethrough',
  n: 'underline',
  o: 'italics',
};

const parseBool = (value?: string | boolean): boolean =>
  typeof value === 'boolean'
    ? value
    : typeof value === 'string'
      ? value.toLowerCase() === 'true'
      : false;

const parseText = (text: string, options: ParseOptions): ParseResult => {
  const result: ParseItem[] = [{ text: '', color: DEFAULT_COLOR }];

  let position = 0;

  while (position + 1 <= text.length) {
    const character = text.charAt(position);

    const item: ParseItem = result[result.length - 1];

    if (character === '\n') {
      result.push({ text: '\n', color: DEFAULT_COLOR });

      position++;

      continue;
    }

    if (character !== options.formattingCharacter) {
      item.text += character;

      position++;

      continue;
    }

    const formattingCode = text.charAt(position + 1).toLowerCase();

    position += 2;

    if (formattingCode === 'r') {
      result.push({ text: '', color: DEFAULT_COLOR });

      continue;
    }

    if (formattingCode in formattingLookupProperties) {
      if (item.text.length > 0) {
        result.push({
          ...item,
          text: '',
          [formattingLookupProperties[formattingCode]]: true,
        });
      } else {
        item[formattingLookupProperties[formattingCode]] = true;
      }
    } else if (formattingCode in colorLookupNames) {
      result.push({ text: '', color: colorLookupNames[formattingCode] });
    }
  }

  return result;
};

const parseChat = (
  chat: Chat,
  options: ParseOptions,
  parent?: Chat
): ParseResult => {
  const result: ParseResult = parseText(
    chat.text || chat.translate || '',
    options
  );

  const item: ParseItem = result[0];

  if (
    (parent && parseBool(parent.bold) && !parseBool(chat.bold)) ||
    parseBool(chat.bold)
  ) {
    item.bold = true;
  }

  if (
    (parent && parseBool(parent.italic) && !parseBool(chat.italic)) ||
    parseBool(chat.italic)
  ) {
    item.italics = true;
  }

  if (
    (parent && parseBool(parent.underlined) && !parseBool(chat.underlined)) ||
    parseBool(chat.underlined)
  ) {
    item.underline = true;
  }

  if (
    (parent &&
      parseBool(parent.strikethrough) &&
      !parseBool(chat.strikethrough)) ||
    parseBool(chat.strikethrough)
  ) {
    item.strikethrough = true;
  }

  if (
    (parent && parseBool(parent.obfuscated) && !parseBool(chat.obfuscated)) ||
    parseBool(chat.obfuscated)
  ) {
    item.obfuscated = true;
  }

  if (chat.color) {
    item.color =
      colorLookupNames[chat.color ?? parent?.color ?? 'white'] || chat.color;
  }

  if (chat.extra) {
    for (const extra of chat.extra) {
      result.push(...parseChat(extra, options, chat));
    }
  }

  return result;
};

export const parse = (
  input: Chat | string,
  options?: ParseOptions
): ParseResult => {
  options = Object.assign(
    {
      formattingCharacter: '§',
    },
    options
  );

  let result;

  switch (typeof input) {
    case 'string': {
      result = parseText(input, options);

      break;
    }
    case 'object': {
      result = parseChat(input, options);

      break;
    }
    default: {
      throw new Error('Unexpected server MOTD type: ' + typeof input);
    }
  }

  return result.filter((item) => item.text.length > 0);
};
