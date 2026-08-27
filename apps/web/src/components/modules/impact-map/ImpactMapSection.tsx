"use client";

import dynamic from "next/dynamic";
import { useState, type ComponentType } from "react";
import { Skeleton } from "@/components/ui/skeleton";

const ImpactMap = dynamic(
  () => import("@/components/organisms/ImpactMap").then((m) => m.ImpactMap),
  {
    ssr: false,
    loading: () => (
      <div className="flex flex-col gap-4">
        <div className="space-y-1">
          <Skeleton className="h-5 w-48 bg-zinc-800" />
          <Skeleton className="h-4 w-72 bg-zinc-800" />
        </div>
        <Skeleton className="h-[300px] md:h-[400px] lg:h-[500px] rounded-xl bj-zinc-900" />
      </div>
    ),
  }
) as ComponentType<{ sortBy?: string }>

export function ImpactMapSection() {
  const [sortBy, setSortBy] = useState("deadline");

  return (
    <div className="space-y-4">
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-white">Impact Projects</h2>
          <select
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value)}
            className="bg-zinc-800 text-sm text-zinc-200 rounded-md px-3 py-1.5 focus:outline-none focus:ring-1 focus:ring-zinc-600"
            aria-label="Sort projects"
          >
            <option value="pay">Highest pay</option>
            <option value="deadline">Soonest deadline</option>
            <option value="altitude">Easiest (lowest altitude)</option>
          </select>
        </div>
        <p className="text-sm text-zinc-400">
          Explore impact projects around the world. Toggle between street and
          satellite views.
        </p>
      </div>
      <ImpactMap sortBy={sortBy} />
    </div>
  );
}
