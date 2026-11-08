// Finds Minecraft players linked to Lodestone users in the player list and in
// console lines. This module has no imports, so its tests run with plain
// `node --test`.

export type LinkedPlayer = {
  minecraft_uuid: string;
  minecraft_name: string;
  username: string;
};

export type LinkedPlayers = {
  /** Lodestone username by lowercase Minecraft name */
  byName: Map<string, string>;
  /** Lodestone username by Minecraft UUID, lowercase with dashes */
  byUuid: Map<string, string>;
};

export const linkedPlayers = (links: LinkedPlayer[]): LinkedPlayers => {
  const byName = new Map<string, string>();
  const byUuid = new Map<string, string>();
  for (const link of links) {
    byName.set(link.minecraft_name.toLowerCase(), link.username);
    byUuid.set(link.minecraft_uuid.toLowerCase(), link.username);
  }
  return { byName, byUuid };
};

/** A Minecraft UUID with or without dashes, lowercase with dashes */
export const normalizeUuid = (uuid: string): string => {
  const hex = uuid.replace(/-/g, '').toLowerCase();
  if (!/^[0-9a-f]{32}$/.test(hex)) return uuid.toLowerCase();
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
};

/** The Lodestone user of a player in the player list */
export const linkedUserOf = (
  linked: LinkedPlayers,
  player: { name: string; uuid?: string | null }
): string | undefined =>
  (player.uuid ? linked.byUuid.get(normalizeUuid(player.uuid)) : undefined) ??
  linked.byName.get(player.name.toLowerCase());

// The messages that name a player at the start of what the server says:
// chat ("<Steve> hi", "[Not Secure] <Steve> hi"), joining and leaving
// ("Steve joined the game", "Steve[/127.0.0.1:5000] logged in ...") and
// /me ("* Steve waves")
const PLAYER_IN_MESSAGE =
  /^(?:\[Not Secure\] )?(?:<([A-Za-z0-9_]{1,16})>|\* ([A-Za-z0-9_]{1,16}) |([A-Za-z0-9_]{1,16})(?: joined the game| left the game|\[\/[^\]]*\] logged in))/;

/**
 * The linked player a console line is about, if any: the player who chats,
 * joins or leaves. Only the message after the log prefix ("[12:00:00]
 * [Server thread/INFO]: ") is read, so a name elsewhere in the line doesn't
 * count.
 */
export const linkedPlayerInLine = (
  plain: string,
  linked: LinkedPlayers
): { player: string; user: string } | null => {
  if (linked.byName.size === 0) return null;
  const colon = plain.indexOf(']: ');
  const message = colon >= 0 ? plain.slice(colon + 3) : plain;
  const match = PLAYER_IN_MESSAGE.exec(message);
  const player = match && (match[1] ?? match[2] ?? match[3]);
  if (!player) return null;
  const user = linked.byName.get(player.toLowerCase());
  return user ? { player, user } : null;
};
