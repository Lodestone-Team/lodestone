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
        // labels: ["pepega1", "pepega2", "pepega3"],
        datasets: [
            {
                data: [10, 20, 30]
            },
        ],
        
    }

    console.log(data);

    return (
        <Card>
            <Line
                datasetIdKey = "cpu"
                data = {data} 
            />
            <Doughnut
                datasetIdKey="ram"
                data = {
                    {
                        labels: ["empty", "usage"],
                        datasets: [
                            {
                                data: [50, 50]
                            },
                        ],
                        
                    }
                }
            />
        </Card>
    );
}

export default SystemMonitor;