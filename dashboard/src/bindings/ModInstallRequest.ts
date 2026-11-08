// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModInstallRequest = {
  version_id: string;
  install_dependencies?: boolean;
  force?: boolean;
};
