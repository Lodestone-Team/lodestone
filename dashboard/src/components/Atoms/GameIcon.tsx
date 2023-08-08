import Tooltip from 'rc-tooltip';
import { Game } from '@bindings/Game';
import {
  game_to_game_icon,
  game_to_game_title,
} from 'data/GameTypeMappings';

export default function GameIcon({
  game_type,
  className = 'h-8 w-8 rounded-sm',
}: {
  game_type: Game;
  className?: string;
}) {
  const game_title = game_to_game_title(game_type);
  const icon = game_to_game_icon(game_type);

  return (
    <Tooltip
      showArrow={false}
      overlay={<span>{game_title}</span>}
      placement="bottom"
      trigger={['hover']}
      mouseEnterDelay={0.2}
    >
      <img src={icon} alt={game_title} className={className} />
    </Tooltip>
  );
}
