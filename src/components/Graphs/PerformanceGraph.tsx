import React, { useState } from 'react';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Filler,
  Legend,
  ChartOptions,
  ChartData,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import { useIntervalImmediate } from 'utils/hooks';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Filler,
  Legend
);

ChartJS.defaults.font = {
  family: 'Satoshi',
  size: 14,
  style: 'normal',
  weight: 'normal',
  lineHeight: 1.2,
}

ChartJS.defaults.color = '#E3E3E4';

export default function PerformanceGraph({
  title,
  color,
  backgroundColor,
  pollrate_s = 1,
  timeWindow_s = 61,
  className = '',
  unit = '',
  getter,
}: {
  title: string;
  color: string;
  backgroundColor: string;
  pollrate_s?: number;
  timeWindow_s?: number;
  className?: string;
  unit?: string;
  getter: () => Promise<[number, number]>;
}): JSX.Element {
  const [max, setMax] = useState(100);
  const labels = Array.from(Array(timeWindow_s / pollrate_s).keys())
    .reverse()
    .map((n) => pollrate_s * n + 's');

  const [dataHistory, setDataHistory] = useState(
    new Array(timeWindow_s / pollrate_s).fill(0)
  );

  const update = async () => {
    const newData = [...dataHistory];
    newData.shift();
    const [value, max] = await getter();
    newData.push(value);
    setMax(max);
    setDataHistory(newData);
  };

  useIntervalImmediate(update, pollrate_s * 1000);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const chartData: ChartData<'line', any[], string> = {
    labels,
    datasets: [
      {
        label: title,
        data: dataHistory,
        backgroundColor: backgroundColor,
        borderColor: color,
        fill: true,
        pointRadius: 0,
        pointHoverRadius: 0,
      },
    ],
  };

  const options: ChartOptions<'line'> = {
    responsive: true,
    plugins: {
      legend: {
        display: false,
      },
      tooltip: {
        enabled: false,
        // callbacks: {
        //   label: (tooltipItem) => tooltipItem.formattedValue + unit,
        // },
      },
      title: {
        display: true,
        text: `${title} - ${dataHistory[dataHistory.length-1]}/${max}${unit}`,
      },
    },
    scales: {
      x: {
        grid: {
          display: true,
          color: '#44464B',
          drawTicks: false,
        },
        ticks: {
          maxRotation: 0,
          maxTicksLimit: 1,
          align: 'start',
          padding: 10,
        },
      },
      y: {
        grid: {
          display: true,
          color: '#44464B',
          drawTicks: false,
        },
        beginAtZero: true,
        min: 0,
        max,
        ticks: {
          maxTicksLimit: 8,
          padding: 10,
        },
      },
    },
    animation: {
      duration: 0,
    },
    interaction: {
      intersect: false,
    },
    elements: {
      point: {
        radius: 0,
      },
    },
  };

  return <Line data={chartData} options={options} className={`${className}`} />;
}
