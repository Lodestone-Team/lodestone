import { MinecraftFlavour } from 'bindings/MinecraftFlavour';

export function flavourStringToMinecraftFlavour(
  flavour: string
): MinecraftFlavour {
  switch (flavour) {
    case 'vanilla':
      return { type: 'vanilla' };
    case 'fabric':
      return { type: 'fabric', loader_version: null, installer_version: null };
    case 'paper':
      return { type: 'paper', build_version: null };
    case 'spigot':
      return { type: 'spigot' };
    case 'forge':
      return { type: 'forge', build_version: null };
  }
  throw new Error(`Unknown flavour: ${flavour}`);
}
