import StatsOverview from "@/components/modules/dashboard/StatsOverview";
import DashboardOverview from "@/components/modules/dashboard/DashboardOverview";
import FeatureCards from "@/components/modules/dashboard/FeatureCards";

const DashboardPage = async () => {
    return (
        <main className="h-full overflow-y-auto space-y-4 md:space-y-12 py-10">
            <StatsOverview />
            <DashboardOverview />
            <FeatureCards />
        </main>
    );
};

export default DashboardPage;
