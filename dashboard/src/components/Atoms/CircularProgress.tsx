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
        pathColor: 'var(--blue-200)',
        trailColor: 'var(--fade-700)',
      })}
      strokeWidth={15}
    />
  );
}
