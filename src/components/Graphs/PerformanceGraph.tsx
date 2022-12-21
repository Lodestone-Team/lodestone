import React, { useEffect, useState } from 'react';
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
  ScriptableLineSegmentContext,
  ScriptableTooltipContext,
  Chart,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import { useIntervalClockSeconds } from 'utils/hooks';
import { asyncCallWithTimeout } from 'utils/util';
import { UnionToIntersection } from 'chart.js/types/utils';

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
  counter: counterProp,
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
  counter?: number;
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
  const [counterState, setCounterState] = useState(data ? 0 : -1);
  const counterRef = React.useRef(counterState);
  counterRef.current = counterState;
  const counter = counterProp ?? counterState;

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
      setCounterState(counterRef.current + 1);
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
        external: externalTooltipHandler,
        mode: 'index',
        intersect: false,
        callbacks: {
          label: (tooltipItem) => tooltipItem.formattedValue + unit,
        },
      },
      title: {
        display: true,
        text: title,
        padding: {
          top: 0,
          bottom: 24,
        },
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
          maxTicksLimit: 100,
          align: 'inner',
          callback: function (val: number | string, idx: number) {
            const num = idx + counter;
            if (idx === 0) return `${timeWindow_s - 1}s`;
            if (idx === timeWindow_s - 1) return '0';
            if (num % 5 !== 0) return null;
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
          maxTicksLimit: 50,
          autoSkip: false,
          padding: 10,
          align: 'end',
          callback: function (val: number | string, idx: number) {
            if (val === 0 || val == displayMax) return val + unit;
            if (idx % 2 !== 1) return null;
            return '';
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

const getOrCreateTooltip = (chart: UnionToIntersection<Chart<'line'>>) => {
  let tooltipEl = chart.canvas.parentNode?.querySelector('div');

  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.style.background = '#1D1E21B0';
    tooltipEl.style.borderRadius = '3px';
    tooltipEl.style.color = '#E3E3E4';
    tooltipEl.style.opacity = '1';
    tooltipEl.style.pointerEvents = 'none';
    tooltipEl.style.position = 'absolute';
    tooltipEl.style.transform = 'translate(-50%, -150%)';
    tooltipEl.style.transition = 'all .1s';
    tooltipEl.style.display = 'flex';
    tooltipEl.style.flexDirection = 'row';
    tooltipEl.style.alignItems = 'flex-start';

    chart.canvas.parentNode?.appendChild(tooltipEl);
  }

  return tooltipEl;
};

const externalTooltipHandler = (context: ScriptableTooltipContext<'line'>) => {
  // Tooltip Element
  const { chart, tooltip } = context;
  const tooltipEl = getOrCreateTooltip(chart);

  // Hide if no tooltip
  if (tooltip.opacity === 0) {
    tooltipEl.style.opacity = '0';
    return;
  }

  // Set Text
  if (tooltip.body) {
    const titleLines = tooltip.title || [];
    const bodyLines = tooltip.body.map((b) => b.lines);

    // Remove old children
    while (tooltipEl.firstChild) {
      tooltipEl.firstChild.remove();
    }

    // Add title lines
    titleLines.forEach((title) => {
      const titleEl = document.createElement('div');
      titleEl.textContent = title + ":";
      titleEl.style.margin = '0 0.25rem 0 0';
      tooltipEl.appendChild(titleEl);
    });

    // Add body lines
    bodyLines.forEach((body, i) => {
      const bodyEl = document.createElement('div');
      bodyEl.textContent = body[0];
      tooltipEl.appendChild(bodyEl);
    });
  }

  const { offsetLeft: positionX, offsetTop: positionY } = chart.canvas;

  // Display, position, and set styles for font
  tooltipEl.style.opacity = '1';
  tooltipEl.style.left = positionX + tooltip.caretX + 'px';
  tooltipEl.style.top = positionY + tooltip.caretY + 'px';
  tooltipEl.style.padding =
    tooltip.options.padding + 'px ' + tooltip.options.padding + 'px';
};
