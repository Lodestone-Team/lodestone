import monaco from 'monaco-editor';
import { negativeLookbehindSupported } from 'utils/util';

type IRichLanguageConfiguration = monaco.languages.LanguageConfiguration;
type ILanguage = monaco.languages.IMonarchLanguage;

export const conf: IRichLanguageConfiguration = {
  comments: {
    lineComment: '#',
  },
  brackets: [
    ['{', '}'],
    ['[', ']'],
    ['(', ')'],
  ],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
  surroundingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};

export const language = <ILanguage>{
  defaultToken: '',
  tokenPostfix: '.toml',

  // we include these common regular expressions
  escapes: /\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,

  // The main tokenizer for our languages
  tokenizer: {
    root: [
      // sections
      // [/^\[[^\]]*\]/, 'metatag'],

      { include: '@comments' },
      { include: '@tables' },
      { include: '@keys' },
      // { include: '@dataTypes' },

      // whitespace
      { include: '@whitespace' },

      // { include: '@float' },
      // { include: '@integer' },

      // strings: recover on non-terminated strings
      // [/"([^"\\]|\\.)*$/, 'string.invalid'], // non-teminated string
      // [/'([^'\\]|\\.)*$/, 'string.invalid'], // non-teminated string
      // [/"/, 'string', '@string."'],
      // [/'/, 'string', "@string.'"],
      { include: '@dateTimeWithTz' },
      { include: '@dateTime' },
      { include: '@date' },
      { include: '@float' },
      { include: '@integer' },
      { include: '@boolean' },
      { include: '@string' },
    ],

    // array: [{ include: '@comments' }, { include: '@dataTypes' }],

    // boolean: [[/(?<!\w)(true|false)(?!\w)/, 'constant.other.boolean.toml']],
    boolean: [[/(true|false)(?!\w)/, 'constant.other.boolean.toml']],

    comments: [
      [
        /\s*((#).*)$/,
        {
          cases: {
            $1: 'comment.line.number-sign.toml',
            $2: 'punctuation.definition.comment.toml',
          },
        },
      ],
    ],

    // dataTypes: [
    //   // { include: '#inlinetable' },
    //   // { include: '#array' },

    //   { include: '@dateTimeWithTz' },
    //   { include: '@dateTime' },
    //   { include: '@date' },
    //   { include: '@float' },
    //   { include: '@integer' },
    //   { include: '@boolean' },
    // ],

    dateTimeWithTz: [
      [
        // /(?<!\w)(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2}))(?!\w)/,
        /(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2}))/,
        'constant.other.datetime-with-timezone.toml',
      ],
    ],

    dateTime: [
      [
        /(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?)/,
        'constant.other.datatime.toml',
      ],
    ],

    date: [[/(\d{4}-\d{2}-\d{2})/, 'constant.other.date.toml']],

    float: [
      [
        // /(?<!\w)([+-]?(0|([1-9](([0-9]|_[0-9])+)?))(?:(?:\.(0|([1-9](([0-9]|_[0-9])+)?)))?[eE][+-]?[1-9]_?[0-9]*|(?:\.[0-9_]*)))(?!\w)/,
        /([+-]?(0|([1-9](([0-9]|_[0-9])+)?))(?:(?:\.(0|([1-9](([0-9]|_[0-9])+)?)))?[eE][+-]?[1-9]_?[0-9]*|(?:\.[0-9_]*)))/,
        'constant.numeric.float.toml',
      ],
    ],

    integer: [
      [
        // /(?<!\w)((?:[+-]?(0|([1-9](([0-9]|_[0-9])+)?))))(?!\w)/,
        /((?:[+-]?(0|([1-9](([0-9]|_[0-9])+)?))))/,
        'constant.numeric.integer.toml',
      ],
    ],

    keys: [[/(^\w+)(\s*)(=)/, ['key', '', 'delimiter']]],

    whitespace: [[/[ \t\r\n]+/, '']],

    string: [
      [/[^\\"']+/, 'string'],
      [/@escapes/, 'string.escape'],
      [/\\./, 'string.escape.invalid'],
      [
        /["']/,
        {
          cases: {
            '$#==$S2': { token: 'string', next: '@pop' },
            '@default': 'string',
          },
        },
      ],
    ],

    tables: [[/^\[\\[^\]]*\]/, 'punctuation.definition.table.toml']],
    // tables: [
    //   [
    //     /^\s*(\[)([^[\]]*)(\])/,
    //     {
    //       cases: {
    //         $1: 'punctuation.definition.table.toml',
    //         $2: [[/[^s.]+/, 'entity.other.attribute-name.table.toml']],
    //         $3: 'punctuation.definition.table.toml',
    //       },
    //     },
    //   ],
    //   [
    //     /^\s*(\[\[)([^[\]]*)(\]\])/,
    //     {
    //       cases: {
    //         $1: 'punctuation.definition.table.array.toml',
    //         $2: [[/[^s.]+/, 'entity.other.attribute-name.table.array.toml']],
    //         $3: 'punctuation.definition.table.array.toml',
    //       },
    //     },
    //   ],
    // ],
  },
};
