import {
    ArcElement,
    CategoryScale,
    Chart as ChartJS,
    Legend,
    LineElement,
    LinearScale,
    PointElement,
    Title,
    Tooltip,
} from 'chart.js';
import { Bar, Doughnut, Line } from "react-chartjs-2";
import React, { useEffect, useRef, useState } from "react";

import Card from "./Card";

ChartJS.register(
    ArcElement,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    Title,
    Tooltip,
    Legend
);

function SystemMonitor() {
    const [cpu, setCpu] = useState([]);

    const data = {
        labels: [1, 2, 3],
        datasets: [
            {
                label: "CPU",
                data: [5, 6, 7],
            },
            {
                label: "RAM",
                data: [3, 2, 1],
            },
        ],
    }

    console.log(data);

    return (
        <Card>
            <Line
                datasetIdKey="label"
                data={data}
                options={{
                    legend: {
                        display: false
                    },
                    tooltips: {
                        callbacks: {
                            label: function (tooltipItem) {
                                return tooltipItem.yLabel;
                            }
                        }
                    }
                }}

            />
            <Doughnut
                datasetIdKey="ram"
                data={
                    {
                        labels: ["usage", "empty"],
                        datasets: [
                            {
                                data: [1535, 124]
                            },
                        ],

                    }
                }
            />
        </Card>
    );
}

export default SystemMonitor;