"use client";

import React, { useState, useEffect, useMemo } from "react";

interface StreamCountdownProps {
    endTime: number;
    status: string;
}

const StreamCountdown: React.FC<StreamCountdownProps> = ({ endTime, status }) => {
    const [currentTime, setCurrentTime] = useState(Date.now());
    const normalizedStatus = status.toLowerCase();

    useEffect(() => {
        if (normalizedStatus !== "active") {
            setCurrentTime(Date.now());
            return;
        }

        setCurrentTime(Date.now());

        const interval = setInterval(() => {
            setCurrentTime(Date.now());
        }, 1000);

        return () => clearInterval(interval);
    }, [normalizedStatus]);

    const timeLeft = useMemo(() => {
        if (normalizedStatus !== "active") return null;

        const diff = endTime - currentTime;
        if (diff <= 0) return null;

        const seconds = Math.floor((diff / 1000) % 60);
        const minutes = Math.floor((diff / (1000 * 60)) % 60);
        const hours = Math.floor((diff / (1000 * 60 * 60)) % 24);
        const days = Math.floor(diff / (1000 * 60 * 60 * 24));

        return {
            days,
            hours,
            minutes,
            seconds,
        };
    }, [currentTime, endTime, normalizedStatus]);

    if (normalizedStatus !== "active") {
        const label = normalizedStatus === "completed"
            ? "Completed"
            : normalizedStatus.charAt(0).toUpperCase() + normalizedStatus.slice(1);
        return <span className="text-zinc-500 font-mono text-xs">{label}</span>;
    }

    if (!timeLeft) {
        return <span className="text-zinc-500 font-mono text-xs">Completed</span>;
    }

    if (timeLeft.days > 0) {
        return (
            <span className="text-white font-mono text-xs">
                {timeLeft.days}d {timeLeft.hours}h {timeLeft.minutes}m remaining
            </span>
        );
    }

    if (timeLeft.hours > 0) {
        return (
            <span className="text-white font-mono text-xs">
                {timeLeft.hours}h {timeLeft.minutes}m {timeLeft.seconds}s remaining
            </span>
        );
    }

    return (
        <span className="text-orange-400 font-mono text-xs animate-pulse">
            {timeLeft.minutes}m {timeLeft.seconds}s remaining
        </span>
    );
};

export default StreamCountdown;
