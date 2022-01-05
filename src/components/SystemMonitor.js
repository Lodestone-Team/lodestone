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
import "./SystemMonitor.css";
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
    const labels = Array.from(Array(24).keys()).reverse().map((n) => 2.5 * n + 2.5);

    const { pollrate, domain, webport } = useContext(ServerContext);

    const [cpuHistory, setCpuHistory] = useState(new Array(24).fill(0));
    const [cpu, setCpu] = useState(0);
    const [mem, setMem] = useState([0, 1]);
    const [disk, setDisk] = useState([0, 1]);


    const getMemUsage = async (domain, webport) => {
        let response = await fetch(`https://${domain}:${webport}/api/sys/mem`);
        const mem = (await response.text()).split("/").map(parseFloat);
        return mem;
    }

    const getDiskUsage = async (domain, webport) => {
        let response = await fetch(`https://${domain}:${webport}/api/sys/disk`);
        const disk = (await response.text()).split("/").map(parseFloat);
        return disk;
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

        getDiskUsage(domain, webport).then(disk => {
            setDisk(disk);
        })
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
        <Card className="systemMonitor">
            <div className="graphWrapper lineGraphWrapper">
                <div className="graph lineGraph">
                    <Line
                        datasetIdKey="cpuHistory"
                        type="line"
                        data={{
                            labels, // TODO need way to map the
                            datasets: [
                                {
                                    data: cpuHistory
                                },
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
                            tooltips: {
                                callbacks: {
                                    label: function (tooltipItem) {
                                        return tooltipItem.yLabel;
                                    }
                                }
                            },
                            animation: false,
                            plugins: {
                                legend: {
                                    display: false
                                },
                            }
                        }}
                    />
                </div>
            </div>
            <div className="graphWrapper doughnutGraphWrapper">
                <p>
                    RAM %
                </p>
                <div className="graph doughnutGraph">
                    <Doughnut
                        datasetIdKey="cpu"
                        data={
                            {
                                labels: ["usage", "available"],
                                datasets: [
                                    {
                                        data: [mem[0] / mem[1], 1 - (mem[0] / mem[1])],
                                        backgroundColor: ['rgb(54, 162, 235)', 'rgb(255, 99, 132)',]
                                    },
                                ],
                            }
                        }
                        options={{
                            animation: false,
                            plugins: {
                                legend: {
                                    display: false
                                },
                            }
                        }}
                    />
                </div>
            </div>
            <div className="graphWrapper doughnutGraphWrapper">
                <p>
                    DISK %
                    <br/>
                    
                </p>
                <div className="graph doughnutGraph">
                    <Doughnut
                        datasetIdKey="mem"
                        data={
                            {
                                labels: ["usage", "available"],
                                datasets: [
                                    {
                                        data: [disk[0] / disk[1], 1 - (disk[0] / disk[1])],
                                        backgroundColor: ['rgb(54, 162, 235)', 'rgb(255, 99, 132)',]
                                    },
                                ],
                            }
                        }
                        options={{
                            animation: false,
                            plugins: {
                                legend: {
                                    display: false
                                },
                            }
                        }}
                    />
                </div>
            </div>
            
        </Card>
    );
}

export default SystemMonitor;