import { useState } from "react";
import { AuthProvider, useAuth } from "./hooks/useAuth";
import TokenAuth from "./components/TokenAuth";
import Layout from "./components/common/Layout";
import TitleBar from "./components/common/TitleBar";
import TournamentList from "./components/tournaments/TournamentList";
import CreateTournament from "./components/tournaments/CreateTournament";
import TournamentDetail from "./components/tournaments/TournamentDetail";
import OfflineTournamentAdmin from "./components/tournaments/OfflineTournamentAdmin";
import HetznerSsh from "./components/HetznerSsh";
import GameExplorer from "./components/GameExplorer";
import PlayerList from "./components/PlayerList";
import MatchManagement from "./components/MatchManagement";
import KycStatus from "./components/KycStatus";
import Dashboard from "./components/Dashboard";
import DeploymentManager from "./components/DeploymentManager";
import Treasury from "./components/Treasury";
import Puzzles from "./components/Puzzles";
import Settings from "./components/Settings";
import TransactionDebugger from "./components/TransactionDebugger";

type Page = "login" | "tournaments" | "create" | "offline" | "detail" | "dashboard" | "hetzner" | "deploy" | "explorer" | "players" | "matches" | "kyc" | "treasury" | "puzzles" | "tx-debug" | "settings";

function AppContent() {
  const { authState, loading } = useAuth();
  const [currentPage, setCurrentPage] = useState<Page>("tournaments");
  const [selectedTournamentId, setSelectedTournamentId] = useState<number | null>(null);

  if (loading) {
    return (
      <AppShell>
        <div style={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          height: "100%",
          backgroundColor: "var(--bg)",
          color: "var(--text-dim)",
        }}>
          Loading...
        </div>
      </AppShell>
    );
  }

  if (!authState.authenticated) {
    return (
      <AppShell>
        <TokenAuth />
      </AppShell>
    );
  }

  // Opens a tournament's detail page: from the list, or straight after creating or
  // duplicating one, so the next-step panel can say what to do.
  const openTournament = (tournamentId: number) => {
    setSelectedTournamentId(tournamentId);
    setCurrentPage("detail");
  };

  const handleBack = () => {
    setCurrentPage("tournaments");
    setSelectedTournamentId(null);
  };


  const renderPage = () => {
    switch (currentPage) {
      case "tournaments":
        return <TournamentList onTournamentSelect={openTournament} />;
      case "create":
        return (
          <CreateTournament
            onTournamentCreated={openTournament}
            onCancel={handleBack}
          />
        );
      case "offline":
        return <OfflineTournamentAdmin />;
      case "detail":
        return selectedTournamentId ? (
          <TournamentDetail
            tournamentId={selectedTournamentId}
            onBack={handleBack}
            onOpenTournament={openTournament}
          />
        ) : null;
      case "dashboard":
        return <Dashboard />;
      case "hetzner":
        return <HetznerSsh />;
      case "deploy":
        return <DeploymentManager />;
      case "explorer":
        return <GameExplorer />;
      case "players":
        return <PlayerList />;
      case "matches":
        return <MatchManagement />;
      case "kyc":
        return <KycStatus />;
      case "treasury":
        return <Treasury />;
      case "puzzles":
        return <Puzzles />;
      case "tx-debug":
        return <TransactionDebugger />;
      case "settings":
        return <Settings />;
      default:
        return null;
    }
  };

  return (
    <AppShell>
      <Layout currentPage={currentPage} onPageChange={(p) => setCurrentPage(p)}>
        {renderPage()}
      </Layout>
    </AppShell>
  );
}

function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <TitleBar />
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>{children}</div>
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
}
