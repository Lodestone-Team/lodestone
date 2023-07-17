import { MinecraftFlavour } from 'bindings/MinecraftFlavour';

export function flavourStringToMinecraftFlavour(
  flavour: string
): MinecraftFlavour {
  switch (flavour.toLowerCase()) {
    case 'vanilla':
      return 'vanilla';
    case 'fabric':
      return { fabric: { loader_version: null, installer_version: null } };
    case 'paper':
      return { paper: { build_version: null } };
    case 'spigot':
      return 'spigot';
    case 'forge':
      return { forge: { build_version: null } };
  }
  throw new Error(`Unknown flavour: ${flavour}`);
}
