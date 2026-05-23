import { useState } from "react";
import { AuthProvider, useAuth } from "./hooks/useAuth";
import TokenAuth from "./components/TokenAuth";
import Layout from "./components/common/Layout";
import TournamentList from "./components/tournaments/TournamentList";
import CreateTournament from "./components/tournaments/CreateTournament";
import TournamentDetail from "./components/tournaments/TournamentDetail";
import HetznerSsh from "./components/HetznerSsh";
import GameExplorer from "./components/GameExplorer";
import PlayerList from "./components/PlayerList";
import MatchManagement from "./components/MatchManagement";
import KycStatus from "./components/KycStatus";
import Dashboard from "./components/Dashboard";
import DeploymentManager from "./components/DeploymentManager";
 
type Page = "login" | "tournaments" | "create" | "detail" | "dashboard" | "hetzner" | "deploy" | "explorer" | "players" | "matches" | "kyc";

function AppContent() {
  const { authState, loading } = useAuth();
  const [currentPage, setCurrentPage] = useState<Page>("tournaments");
  const [selectedTournamentId, setSelectedTournamentId] = useState<number | null>(null);

  if (loading) {
    return (
      <div style={{
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        minHeight: "100vh",
        backgroundColor: "#1a1a1a",
        color: "#999",
      }}>
        Loading...
      </div>
    );
  }

  if (!authState.authenticated) {
    return <TokenAuth onAuth={() => {
      // Auth is handled by the AuthProvider
    }} />;
  }

  const handleTournamentSelect = (tournamentId: number) => {
    setSelectedTournamentId(tournamentId);
    setCurrentPage("detail");
  };

  const handleTournamentCreated = () => {
    setCurrentPage("tournaments");
  };

  const handleBack = () => {
    setCurrentPage("tournaments");
    setSelectedTournamentId(null);
  };

  const handleEdit = (tournamentId: number) => {
    setSelectedTournamentId(tournamentId);
    setCurrentPage("create");
  };

  const renderPage = () => {
    switch (currentPage) {
      case "tournaments":
        return <TournamentList onTournamentSelect={handleTournamentSelect} />;
      case "create":
        return (
          <CreateTournament
            onTournamentCreated={handleTournamentCreated}
            onCancel={handleBack}
          />
        );
      case "detail":
        return selectedTournamentId ? (
          <TournamentDetail
            tournamentId={selectedTournamentId}
            onBack={handleBack}
            onEdit={handleEdit}
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
      default:
        return null;
    }
  };

  return (
    <Layout currentPage={currentPage} onPageChange={(p) => setCurrentPage(p)}>
      {renderPage()}
    </Layout>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
}
