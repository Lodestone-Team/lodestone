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
  Tick,
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
};

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
  const [counter, setCounter] = useState(0);

  const update = async () => {
    const newData = [...dataHistory];
    newData.shift();
    const [value, max] = await getter();
    newData.push(value);
    setMax(max);
    setDataHistory(newData);
    setCounter(counter + 1);
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
        mode: 'index',
        intersect: false,
        callbacks: {
          label: (tooltipItem) => tooltipItem.formattedValue + unit,
        },
        filter: (tooltipItem) => tooltipItem.parsed.y !== 0,
      },
      title: {
        display: true,
        text: `${title} - ${dataHistory[dataHistory.length - 1]}/${max}${unit}`,
      },
    },
    scales: {
      x: {
        grid: {
          display: true,
          color: function (context) {
            return context.tick.value === 0 || context.tick.value === 60
              ? '#767A82'
              : '#44464B';
          },
          lineWidth: function (context) {
            return context.tick.value === 0 || context.tick.value === 60
              ? 2
              : 1;
          },
          drawTicks: false,
        },
        ticks: {
          maxRotation: 0,
          padding: 10,
          align: 'inner',
          callback: function (val: number | string, idx: number) {
            const num = idx + counter;
            if (idx === 0) return `${timeWindow_s - 1}s`;
            if (idx === timeWindow_s - 1) return '0';
            if (num % 4 !== 0) return null;
            return '';
          },
        },
      },
      y: {
        grid: {
          display: true,
          color: function (context) {
            return context.tick.value === 0 || context.tick.value === max
              ? '#767A82'
              : '#44464B';
          },
          lineWidth: function (context) {
            return context.tick.value === 0 || context.tick.value === max
              ? 2
              : 1;
          },
          drawTicks: false,
        },
        beginAtZero: true,
        min: 0,
        max,
        ticks: {
          maxTicksLimit: 4,
          padding: 10,
          align: 'inner',
          callback: (value) => value + unit,
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
