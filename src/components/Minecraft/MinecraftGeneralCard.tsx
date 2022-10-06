import axios from 'axios';
import DashboardCard from 'components/DashboardCard';
import Textfield from 'components/Textfield';
import { updateInstance } from 'data/InstanceList';
import { axiosPutSingleValue, axiosWrapper } from 'utils/util';
import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';

export default function MinecraftGeneralCard({
  instance,
}: {
  instance: InstanceInfo;
}) {
  const queryClient = useQueryClient();

  return (
    <DashboardCard>
      <h1 className="font-bold text-medium"> General Settings </h1>
      <div className="flex flex-row">
        <Textfield
          label="Port"
          value={instance.port.toString()}
          onSubmit={async (port) => {
            const numPort = parseInt(port);
            await axiosPutSingleValue<void>(
              `/instance/${instance.uuid}/port`,
              numPort
            );
            updateInstance(instance.uuid, queryClient, (oldData) => ({
              ...oldData,
              port: numPort,
            }));
          }}
          validate={async (port) => {
            const numPort = parseInt(port);
            if (isNaN(numPort)) throw new Error('Port must be a number');
            if (numPort < 0 || numPort > 65535)
              throw new Error('Port must be between 0 and 65535');
            const result = await axiosWrapper<boolean>({
              method: 'get',
              url: `/check/port/${numPort}`,
            });
            if (result) throw new Error('Port not available');
          }}
        />
      </div>
    </DashboardCard>
  );
}
