import { Bar, Doughnut, Line } from "react-chartjs-2";
import {
    ArcElement,
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    Title,
    Tooltip,
    Legend,
  } from 'chart.js';
import React, { useEffect, useState, useRef } from "react";
import Card from "../screens/Card";

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