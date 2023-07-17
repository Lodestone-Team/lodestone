import { CircularProgressbar, buildStyles } from 'react-circular-progressbar';
import 'react-circular-progressbar/dist/styles.css';

export default function CircularProgress({
  progress_percent = 0,
}: {
  progress_percent?: number;
}) {
  return (
    <CircularProgressbar
      value={progress_percent * 100}
      styles={buildStyles({
        pathColor: '#59B2F3',
        trailColor: 'rgba(209, 209, 218, 0.1)',
      })}
      strokeWidth={15}
    />
  );
}
