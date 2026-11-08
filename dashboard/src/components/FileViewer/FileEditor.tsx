import Editor, { loader, useMonaco } from '@monaco-editor/react';
import { useQueryClient } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { InstanceContext } from 'data/InstanceContext';
import type * as monaco from 'monaco-editor';
import { useContext, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useIsomorphicLayoutEffect } from 'usehooks-ts';
import { saveInstanceFile } from 'utils/apis';
import * as toml from 'utils/monaco-languages/toml';
import { useResolvedTheme } from 'utils/theme';
import { SkeletonCode, SkeletonGroup } from 'components/Atoms/Skeleton';

type Monaco = typeof monaco;

// Load monaco from our own origin instead of the jsdelivr CDN (the loader's
// default). public/monaco/vs is copied from the installed monaco-editor
// package by scripts/copy-monaco.mjs (run by `npm run dev` / `npm run build`).
loader.config({ paths: { vs: '/monaco/vs' } });

// monaco is a page-wide singleton, so the theme and the toml language only
// need to be registered once, not on every editor mount
let monacoConfigured = false;

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
  const fileContentRef = useRef<string | undefined>(undefined);
  fileContentRef.current = fileContent;
  const originalFileContentRef = useRef(originalFileContent);
  useIsomorphicLayoutEffect(() => {
    originalFileContentRef.current = originalFileContent;
  }, [originalFileContent]);

  const monaco = useMonaco();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const { selectedInstance: instance } = useContext(InstanceContext);
  const theme = useResolvedTheme();

  function handleEditorDidMount(
    editor: monaco.editor.IStandaloneCodeEditor,
    monaco: Monaco
  ) {
    editorRef.current = editor;
    // Monaco may have reused a model left over for this path, which holds
    // whatever text it had last time. Reset it to the content just loaded from
    // the server, so stale text can't be saved over a newer file.
    const freshContent = originalFileContentRef.current ?? '';
    const model = editor.getModel();
    if (model && model.getValue() !== freshContent)
      model.setValue(freshContent);
    setFileContent(freshContent);
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
    if (monaco && !monacoConfigured) {
      monacoConfigured = true;
      monaco.editor.defineTheme('lodestone-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [],
        colors: {
          'editor.background': '#26282C',
          'editor.lineHighlightBackground': '#2c2e33',
        },
      });
      // a white page with the light theme's hover gray for the current line
      monaco.editor.defineTheme('lodestone-light', {
        base: 'vs',
        inherit: true,
        rules: [],
        colors: {
          'editor.background': '#FFFFFF',
          'editor.lineHighlightBackground': '#F1F4F7',
          'editorGutter.background': '#F3F5F8',
        },
      });
      // monaco 0.55 moved languages.typescript to the top level typescript namespace
      monaco.typescript.typescriptDefaults.setCompilerOptions({
        target: monaco.typescript.ScriptTarget.ES2016,
        allowNonTsExtensions: true,
        allowJs: true,
        allowSyntheticDefaultImports: true,
        moduleResolution: monaco.typescript.ModuleResolutionKind.NodeJs,
        module: monaco.typescript.ModuleKind.ESNext,
        esModuleInterop: true,
      });
      monaco.typescript.typescriptDefaults.setEagerModelSync(true);
      monaco.languages.register({ id: 'toml' });
      monaco.languages.setLanguageConfiguration('toml', toml.conf);
      if (toml.language)
        monaco.languages.setMonarchTokensProvider('toml', toml.language);
      // monaco.typescript.typescriptDefaults.setDiagnosticsOptions({
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
      theme={theme === 'light' ? 'lodestone-light' : 'lodestone-dark'}
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
      // shown only if the editor takes a while to start (its first load):
      // lines of code, appearing after a moment so a quick start shows nothing
      loading={
        <SkeletonGroup label="Loading editor" className="size-full p-4">
          <SkeletonCode lines={16} />
        </SkeletonGroup>
      }
      // dispose the model when the file is closed instead of keeping it
      keepCurrentModel={false}
    />
  );
}
