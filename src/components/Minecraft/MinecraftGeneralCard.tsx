import axios from 'axios';
import DashboardCard from 'components/DashboardCard';
import Textfield from 'components/Textfield';
import { ClientError } from 'data/ClientError';
import { InstanceInfo, updateInstance } from 'data/InstanceList';
import { axiosPutSingleValue, axiosWrapper } from 'utils/util';
import { Result } from '@badrap/result';
import { useQueryClient } from '@tanstack/react-query';

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
            if (isNaN(numPort))
              return Result.err(ClientError.fromString('must be a number'));
            if (numPort < 0 || numPort > 65535)
              return Result.err(
                ClientError.fromString('must be between 0 and 65535')
              );
            const result = await axiosPutSingleValue<void>(
              `/instance/${instance.uuid}/port`,
              numPort
            );
            if (result.isOk) {
              updateInstance(instance.uuid, queryClient, (oldData) => ({
                ...oldData,
                port: numPort,
              }));
            }
            return result;
          }}
          validate={async (port) => {
            const numPort = parseInt(port);
            if (isNaN(numPort))
              return Result.err(ClientError.fromString('must be a number'));
            if (numPort < 0 || numPort > 65535)
              return Result.err(
                ClientError.fromString('must be between 0 and 65535')
              );
            const result = await axiosWrapper<boolean>({
              method: 'get',
              url: `/check/port/${numPort}`,
            });
            if (result.isOk && result.unwrap() === true) {
              return Result.err(
                ClientError.fromString('port is already in use')
              );
            }
            return result;
          }}
        />
      </div>
    </DashboardCard>
  );
}
