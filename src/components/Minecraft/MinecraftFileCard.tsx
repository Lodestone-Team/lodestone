import DashboardCard from 'components/DashboardCard';
import InstanceCard from 'components/InstanceCard';
import Editor, { DiffEditor, useMonaco, loader } from '@monaco-editor/react';

export default function MinecraftFileCard() {
  return (
    <DashboardCard>
      <Editor
        height="50vh"
        defaultValue="// some comment"
        theme="vs-dark"
        path="file.properties"
      />
    </DashboardCard>
  );
}
