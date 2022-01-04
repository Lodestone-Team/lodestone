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

    const { pollrate, domain, webport } = useContext(ServerContext);

    const [cpuHistory, setCpuHistory] = useState(new Array(61).fill(0));
    const [cpu, setCpu] = useState(0)
    const [mem, setMem] = useState(0);


    const getMemUsage = async (domain, webport) => {
        let response = await fetch(`https://${domain}:${webport}/api/sys/mem`);
        const split = (await response.text()).split("/");
        let memUsage = parseFloat(split[0]) / parseFloat(split[1]);
        return memUsage;
    }
    
    const getCpuUsage = async (domain, webport) => {
        let response = await fetch(`https://${domain}:${webport}/api/sys/cpuutil`);
        let cpuUsage = parseFloat(await response.text())
        return cpuUsage*100;
    }

    const update = (domain, webport) => {
        const newCpuHistory = [...cpuHistory];
        newCpuHistory.shift()

        getCpuUsage(domain, webport).then(cpu => {
            setCpu(cpu);
            newCpuHistory.push(cpu);
            setCpuHistory(newCpuHistory);
        });

        getMemUsage(domain, webport).then(mem => {
            setMem(mem);
        });
    }

    useInterval(() => { update(domain, webport) }, pollrate, true)

    const data = {
        labels, // TODO need way to map the 
        datasets: [
            {
                data: cpuHistory
            },
            // {
            //     xAxisID: "x",
            //     yAxisID: "y",
            //     label: "CPU",
            //     data: [50, 60, 70, 80, 90, 100],
            // },
        ],
    }
    
    return (
        <Card>
            <Line
                datasetIdKey="cpuHistory"
                type="line"
                data={{
                    labels, // TODO need way to map the 
                    datasets: [
                        {
                            data: cpuHistory
                        },
                        // {
                        //     xAxisID: "x",
                        //     yAxisID: "y",
                        //     label: "CPU",
                        //     data: [50, 60, 70, 80, 90, 100],
                        // },
                    ],
                }}
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
                datasetIdKey="cpu"
                data={
                    {
                        labels: ["usage", "available"],
                        datasets: [
                            {
                                data: [cpu, 1 - cpu],
                                backgroundColor: ['rgb(54, 162, 235)', 'rgb(255, 99, 132)',]
                            },
                        ],

                    }
                }
                options={{
                    animation: false,
                }}
            />
            <Doughnut
                datasetIdKey="mem"
                data={
                    {
                        labels: ["usage", "available"],
                        datasets: [
                            {
                                data: [mem, 1 - mem],
                                backgroundColor: ['rgb(54, 162, 235)', 'rgb(255, 99, 132)',]
                            },
                        ],

                    }
                }
                options={{
                    animation: false,
                }}
            />
        </Card>
    );
}

export default SystemMonitor;