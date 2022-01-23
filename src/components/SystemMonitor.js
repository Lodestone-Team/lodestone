import "./SystemMonitor.css";

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
import { Doughnut, Line } from "react-chartjs-2";
import React, { useContext, useState } from "react";

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
    const { pollrate, api_domain, api_path } = useContext(ServerContext);
    const pollrate_s = pollrate / 1000.0;
    const bytesInGigabyte = 1048576;
    const labels = Array.from(Array(60 / pollrate_s).keys()).reverse().map((n) => pollrate_s * n + pollrate_s + "s");


    const [cpuHistory, setCpuHistory] = useState(new Array(60 / pollrate_s).fill(0));
    // eslint-disable-next-line no-unused-vars
    const [cpu, setCpu] = useState(0);
    const [mem, setMem] = useState([0, 1]);
    const [disk, setDisk] = useState([0, 1]);

    const byteToGigabyte = (x) => {
        return Math.round(x / bytesInGigabyte * 10) / 10;
    }

    const getMemUsage = async () => {
        let response = await fetch(`${api_domain}${api_path}/sys/mem`);
        const mem = (await response.text()).split("/").map(parseFloat);
        return mem;
    }

    const getDiskUsage = async () => {
        let response = await fetch(`${api_domain}${api_path}/sys/disk`);
        const disk = (await response.text()).split("/").map(parseFloat);
        return disk;
    }

    const getCpuUsage = async () => {
        let response = await fetch(`${api_domain}${api_path}/sys/cpuutil`);
        let cpuUsage = parseFloat(await response.text())
        return cpuUsage * 100;
    }

    const update = () => {
        const newCpuHistory = [...cpuHistory];
        newCpuHistory.shift()

        getCpuUsage().then(cpu => {
            setCpu(cpu);
            newCpuHistory.push(cpu);
            setCpuHistory(newCpuHistory);
        });

        getMemUsage().then(mem => {
            setMem(mem);
        });

        getDiskUsage().then(disk => {
            setDisk(disk);
        })
    }

    useInterval(() => { update() }, pollrate, true)

    return (
        <Card className="systemMonitor">

            <div className="graphWrapper lineGraphWrapper">
                <p>
                    CPU %
                </p>
                <div className="graph lineGraph">
                    <Line
                        datasetIdKey="cpuHistory"
                        type="line"
                        data={{
                            labels, // TODO need way to map the
                            datasets: [
                                {
                                    backgroundColor: "rgba(0, 143, 6, 0.5)",
                                    data: cpuHistory,
                                    tension: 0.4,
                                    pointRadius: 0,
                                    fill: true,
                                    borderColor: "rgba(0, 143, 6, 1)",
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
                    RAM
                    <br />
                    {byteToGigabyte(mem[0])}/{byteToGigabyte(mem[1])} GB
                    <br />
                    Used
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
                    DISK
                    <br />
                    {byteToGigabyte(disk[0])}/{byteToGigabyte(disk[1])} GB
                    <br />
                    Used
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