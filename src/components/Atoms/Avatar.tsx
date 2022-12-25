import BoringAvatar, { AvatarProps } from 'boring-avatars';

const Avatar = ({
  name,
  size = 20,
  variant = 'beam',
  colors = ['#62DD76', '#1D8EB2', '#EFB440', '#DD6262', '#dd62c6'],
  ...props
}: AvatarProps) => {
  return (
    <BoringAvatar
      name={name}
      size={size}
      variant={variant}
      colors={colors}
      {...props}
    />
  );
};

export default Avatar;
