import InputField from 'components/Atoms/Form/InputField';
import Textfield from 'components/Atoms/Config/TextBox';

export default function MinecraftNameForm() {
  return (
    <>
      <h1 className="font-bold tracking-tight text-gray-300 text-larger">
        Create an Instance
      </h1>
      <p>Create a new Minecraft server instance to play with your friends.</p>
      <div className="flex flex-col gap-8 mt-10 text-left">
        <InputField type="text" name="name" label="Name" />
      </div>
    </>
  );
}
