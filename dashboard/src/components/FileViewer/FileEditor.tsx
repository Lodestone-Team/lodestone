import Editor, { useMonaco } from '@monaco-editor/react';
import { useQueryClient } from '@tanstack/react-query';
import { ClientFile } from '@bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import { useContext, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useIsomorphicLayoutEffect } from 'usehooks-ts';
import { saveInstanceFile } from 'utils/apis';
import * as toml from 'utils/monaco-languages/toml';

type Monaco = typeof monaco;

// A wrapper of monaco editor for file editing, with toml support and ctrl+s
export function FileEditor({
  file,
  path,
  monacoPath,
  originalFileContent,
  fileContent,
  setFileContent,
}: {
  file: ClientFile | undefined;
  path: string;
  monacoPath: string;
  originalFileContent: string | undefined;
  fileContent: string;
  setFileContent: (content: string) => void;
}) {
  const queryClient = useQueryClient();
  const fileContentRef = useRef<string>();
  fileContentRef.current = fileContent;

  const monaco = useMonaco();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const { selectedInstance: instance } = useContext(InstanceContext);

  function handleEditorDidMount(
    editor: monaco.editor.IStandaloneCodeEditor,
    monaco: Monaco
  ) {
    editorRef.current = editor;
    // add ctrl+s save
    if (!instance) return;
    if (!file) return;
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () =>
      saveInstanceFile(
        instance.uuid,
        path,
        file,
        fileContentRef.current || '',
        queryClient
      )
    );
    setFileContent(editor.getValue());
  }

  // for overwriting the file type for certain files
  const monacoLanguage = monacoPath.endsWith('.lodestone_config')
    ? 'json'
    : monacoPath.endsWith('.toml')
    ? 'toml'
    : undefined;

  useIsomorphicLayoutEffect(() => {
    // set monaco theme, just a different background color
    // also set some ts settings
    if (monaco) {
      monaco.editor.defineTheme('lodestone-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [],
        colors: {
          'editor.background': '#26282C',
          'editor.lineHighlightBackground': '#2c2e33',
        },
      });
      monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
        target: monaco.languages.typescript.ScriptTarget.ES2016,
        allowNonTsExtensions: true,
        allowJs: true,
        allowSyntheticDefaultImports: true,
        moduleResolution:
          monaco.languages.typescript.ModuleResolutionKind.NodeJs,
        module: monaco.languages.typescript.ModuleKind.ESNext,
        esModuleInterop: true,
      });
      monaco.languages.typescript.typescriptDefaults.setEagerModelSync(true);
      monaco.languages.register({ id: 'toml' });
      monaco.languages.setLanguageConfiguration('toml', toml.conf);
      if (toml.language)
        monaco.languages.setMonarchTokensProvider('toml', toml.language);
      // monaco.languages.typescript.typescriptDefaults.setDiagnosticsOptions({
      //   noSemanticValidation: true,
      //   noSyntaxValidation: true,
      // });
    }
  }, [monaco]);

  return (
    <Editor
      height="100%"
      onChange={(value) => {
        setFileContent(value ?? '');
      }}
      value={fileContent}
      defaultValue={originalFileContent}
      theme="lodestone-dark"
      path={monacoPath}
      className="bg-gray-800"
      options={{
        padding: {
          top: 8,
        },
        minimap: {
          enabled: false,
        },
        wordWrap: 'on',
      }}
      language={monacoLanguage}
      saveViewState={true}
      onMount={handleEditorDidMount}
      keepCurrentModel={true}
    />
  );
}
