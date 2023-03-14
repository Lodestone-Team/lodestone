import { GameType, Games, GameVariants } from 'bindings/InstanceInfo';
import Tooltip from 'rc-tooltip';
import { gameIcons, spanMap } from 'data/GameTypeMappings';
const unknown_icon = '/assets/minecraft-missing-texture.svg';

export default function GameIcon({
  game_type,
  className = 'h-8 w-8 rounded-sm',
}: {
  game_type: GameType;
  className?: string;
}) {
  let icon = unknown_icon;
  let span = '';
  const game = Object.keys(game_type)[0] as Games;
  const variant = game_type[game]['variant'] as GameVariants;

  if (game in gameIcons) {
    if (variant in gameIcons[game]) {
      icon = gameIcons[game][variant];
      span = spanMap[game][variant];
    }
  }

  return (
    <Tooltip
      showArrow={false}
      overlay={<span>{span}</span>}
      placement="bottom"
      trigger={['hover']}
      mouseEnterDelay={0.2}
    >
      <img src={icon} alt={variant} className={`${className}`} />
    </Tooltip>
  );
}
