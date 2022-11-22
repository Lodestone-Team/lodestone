const game_icons: { [key: string]: { [key: string]: string } } = {
  minecraft: {
    vanilla: '/assets/minecraft-vanilla.png',
    fabric: '/assets/minecraft-fabric.png',
  },
};

const unknown_icon = '/assets/minecraft-missing-texture.svg';

export default function GameIcon({
  game_type,
  game_flavour,
  className = 'h-8 w-8',
}: {
  game_type: string;
  game_flavour: string;
  className?: string;
}) {
  let icon = unknown_icon;
  if (game_type in game_icons)
    if (game_flavour in game_icons[game_type])
      icon = game_icons[game_type][game_flavour];
  return (
    <img
      src={icon}
      alt={game_type}
      className={`${className}`}
    />
  );
}
