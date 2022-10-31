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
  ScriptableLineSegmentContext,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import { useIntervalClockSeconds, useIntervalImmediate } from 'utils/hooks';
import { asyncCallWithTimeout } from 'utils/util';

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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const skipped = (ctx: ScriptableLineSegmentContext, value: any) =>
  ctx.p0.skip || ctx.p1.skip ? value : undefined;

/**
 *
 * @param getter A function that returns [value, max], if this is set to null, the graph will not pull data by itself and will rely on data being passed in
 */
export default function PerformanceGraph({
  title,
  color,
  backgroundColor,
  data,
  max: maxProp,
  pollrate_s = 1,
  timeWindow_s = 61,
  className = '',
  unit = '',
  getter,
}: {
  title: string;
  color: string;
  backgroundColor: string;
  data?: number[];
  max?: number;
  pollrate_s?: number;
  timeWindow_s?: number;
  className?: string;
  unit?: string;
  getter?: () => Promise<[number, number]>;
}): JSX.Element {
  const [max, setMax] = useState(100);
  const labels = Array.from(Array(timeWindow_s / pollrate_s).keys())
    .reverse()
    .map((n) => pollrate_s * n + 's');

  const [dataHistory, setDataHistory] = useState(
    new Array(timeWindow_s / pollrate_s).fill(NaN)
  );
  const dataRef = React.useRef(dataHistory);
  dataRef.current = dataHistory;
  const [counter, setCounter] = useState(-1);
  const counterRef = React.useRef(counter);
  counterRef.current = counter;

  const update = async () => {
    if (!getter) return;
    const now = Date.now();
    let val = NaN;
    try {
      const [value, max] = await asyncCallWithTimeout(
        getter(),
        pollrate_s * 1000 * 0.9
      );
      val = value;
      if (max) setMax(max);
    } catch (e) {
      console.log(e);
    }

    setTimeout(() => {
      const newHistory = [...dataRef.current];
      newHistory.shift();
      newHistory.push(val);
      setDataHistory(newHistory);
      setCounter(counterRef.current + 1);
    }, Math.max(0, pollrate_s * 1000 - (Date.now() - now)));
  };

  useIntervalClockSeconds(update, pollrate_s);

  const displayData = data ?? dataHistory;
  const displayMax = maxProp ?? max;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const chartData: ChartData<'line', any[], string> = {
    labels,
    datasets: [
      {
        label: title,
        data: displayData,
        backgroundColor: backgroundColor,
        borderColor: color,
        fill: true,
        pointRadius: 0,
        pointHoverRadius: 0,
        segment: {
          borderColor: (ctx) => skipped(ctx, '#767A82'),
          borderDash: (ctx) => skipped(ctx, [6, 6]),
          backgroundColor: (ctx) => skipped(ctx, '#A5A5AC4D'),
        },
        spanGaps: true,
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
        text: `${title} - ${
          displayData[displayData.length - 1]
        }/${displayMax}${unit}`,
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
            return context.tick.value === 0 || context.tick.value === displayMax
              ? '#767A82'
              : '#44464B';
          },
          lineWidth: function (context) {
            return context.tick.value === 0 || context.tick.value === displayMax
              ? 2
              : 1;
          },
          drawTicks: false,
        },
        beginAtZero: true,
        // min: 0,
        max: displayMax,
        ticks: {
          maxTicksLimit: 8,
          padding: 10,
          align: 'inner',
          callback: function (val: number | string, idx: number) {
            return val + unit;
          },
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
