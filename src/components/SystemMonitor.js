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
import React, { useContext, useEffect, useRef, useState } from "react";

import Card from "./Card";
import { ServerContext } from "../contexts/ServerContext";
import { useInterval } from '../utils';

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
    const labels = Array.from(Array(61).keys()).reverse();

    console.log(labels);
    
    const { pollrate, domain, webport } = useContext(ServerContext);

    const [cpuHistory, setCpuHistory] = useState(new Array(61).fill(0));
    const [ram, setRam] = useState(new Array(61).fill(0));


    // return just a number/tuple idfk
    const getCurCpuUsage = async (domain, webport) => {
        // TODO fetch backend for data
        // let response = await fetch(`https://${domain}:${webport}/api/sys/`);
        // TODO process data to get a percentage back
        return 0;
    }

    const updateCPU = (domain, webport) => {
        const newCpu = [...cpuHistory];
        newCpu.shift()
        
        const newCpuUsage = getCurCpuUsage(domain, webport);
        newCpu.push(newCpuUsage);
        setCpuHistory(newCpu)
    }

    useInterval(updateCPU(domain, webport), pollrate, true)

    const data = {
        labels, // TODO need way to map the 
        datasets: [
            {
                xAxisID: "x",
                yAxisID: "y",
                label: "CPU",
                data: [50, 60, 70, 80, 90, 100],
            },
            {
                xAxisID: "x",
                yAxisID: "y",
                label: "RAM",
                data: [30, 20, 10, 0, 10, 20],
            },
        ],
    }

    console.log(data);

    return (
        <Card>
            <Line
                datasetIdKey="cpu"
                type="line"
                data={data}
                options={{
                    scales: {
                        x: {
                            ticks: {
                                maxTicksLimit: 2,
                                autoSkip: 5,
                            }
                        },
                        y: {
                            beginAtZero: true,
                            min: 0,
                            max: 100,
                            ticks: {
                                stepSize: 25,
                            }

                        }
                    },
                    legend: {
                        display: false
                    },
                    tooltips: {
                        callbacks: {
                            label: function (tooltipItem) {
                                return tooltipItem.yLabel;
                            }
                        }
                    },
                    animation: false
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