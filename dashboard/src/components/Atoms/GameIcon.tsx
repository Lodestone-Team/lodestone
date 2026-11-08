import Tooltip from 'rc-tooltip';
import { Game } from 'bindings/Game';
import { game_to_game_icon, game_to_game_title } from 'data/GameTypeMappings';
import { useState } from 'react';
import { modpackIconUrl } from 'utils/mods';
import { cn } from 'utils/util';
import RemoteIcon from './RemoteIcon';

/**
 * An instance's game icon, or its modpack's icon when it has one
 * (`InstanceInfo.icon_url`). The modpack icon comes from Modrinth's CDN; if
 * it is not a CDN URL or fails to load, the game icon is shown instead.
 */
export default function GameIcon({
  game_type,
  icon_url,
  className = 'h-8 w-8 rounded-xs',
}: {
  game_type: Game;
  icon_url?: string | null;
  className?: string;
}) {
  const game_title = game_to_game_title(game_type);
  const modpackIcon = modpackIconUrl(icon_url);
  const [failedIcon, setFailedIcon] = useState<string | null>(null);
  const showModpack = modpackIcon !== null && modpackIcon !== failedIcon;

  return (
    <Tooltip
      showArrow={false}
      overlay={<span>{game_title}</span>}
      placement="bottom"
      trigger={['hover']}
      mouseEnterDelay={0.2}
    >
      {showModpack ? (
        <RemoteIcon
          key="modpack"
          src={modpackIcon}
          alt={game_title}
          // same size as the game icon, as a rounded square
          className={cn(className, 'rounded-xs object-cover')}
          onError={() => setFailedIcon(modpackIcon)}
        />
      ) : (
        <img
          key="game"
          src={game_to_game_icon(game_type)}
          alt={game_title}
          className={className}
        />
      )}
    </Tooltip>
  );
}
