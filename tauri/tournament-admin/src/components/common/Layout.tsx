import { useState } from "react";
import { useAuth } from "../../hooks/useAuth";

interface LayoutProps {
  children: React.ReactNode;
  currentPage?: string;
  onPageChange?: (page: any) => void;
}

export default function Layout({ children, currentPage = "dashboard", onPageChange }: LayoutProps) {
  const { logout } = useAuth();
  const [sidebarOpen, setSidebarOpen] = useState(true);

  const menuItems = [
    { id: "dashboard", label: "Dashboard", icon: "" },
    { id: "tournaments", label: "Tournaments", icon: "" },
    { id: "create", label: "Create Tournament", icon: "" },
    { id: "hetzner", label: "Hetzner Server", icon: "️" },
    { id: "matches", label: "Match Management", icon: "" },
    { id: "players", label: "Players", icon: "" },
    { id: "kyc", label: "KYC Status", icon: "" },
    { id: "deploy", label: "Deployment", icon: "🚀" },
    { id: "explorer", label: "Game Explorer", icon: "👁️" },
    { id: "treasury", label: "Treasury", icon: "💎" },
    { id: "settings", label: "Settings", icon: "⚙️" },
  ];

  return (
    <div style={{
      display: "flex",
      minHeight: "100vh",
      width: "100%",
      backgroundColor: "var(--bg)",
      fontFamily: "'Outfit', sans-serif",
      overflowX: "hidden",
      position: "relative",
    }}>
      <div className="onboarding-bg" />
      
      {/* Sidebar */}
      <div style={{
        width: sidebarOpen ? "250px" : "80px",
        backgroundColor: "rgba(10, 33, 26, 0.7)",
        backdropFilter: "blur(20px)",
        borderRight: "1px solid var(--border)",
        transition: "width 0.4s cubic-bezier(0.16, 1, 0.3, 1)",
        overflow: "hidden",
        zIndex: 10,
        margin: "1rem",
        borderRadius: "24px",
        height: "calc(100vh - 2rem)",
      }}>
        <div style={{
          padding: "1rem",
          borderBottom: "1px solid #404040",
        }}>
          <div style={{
            display: "flex",
            alignItems: "center",
            justifyContent: sidebarOpen ? "space-between" : "center",
          }}>
            {sidebarOpen && (
              <h3 style={{
                color: "#ad5c2f",
                margin: 0,
                fontSize: "16px",
              }}>
                Admin Panel
              </h3>
            )}
            <button
              onClick={() => setSidebarOpen(!sidebarOpen)}
              style={{
                background: "none",
                border: "none",
                color: "#999",
                cursor: "pointer",
                fontSize: "18px",
                padding: "4px",
              }}
            >
              {sidebarOpen ? "◀" : "▶"}
            </button>
          </div>
        </div>

        <nav style={{
          padding: "1rem 0",
        }}>
          {menuItems.map((item) => (
            <div
              key={item.id}
              onClick={() => onPageChange?.(item.id)}
              style={{
                display: "flex",
                alignItems: "center",
                padding: sidebarOpen ? "0.75rem 1rem" : "0.75rem",
                margin: "0.25rem 0.75rem",
                borderRadius: "100px",
                cursor: "pointer",
                backgroundColor: currentPage === item.id ? "var(--primary)" : "transparent",
                color: currentPage === item.id ? "#ffffff" : "var(--text-dim)",
                transition: "all 0.2s ease",
                border: currentPage === item.id ? "1px solid rgba(255,255,255,0.1)" : "1px solid transparent",
              }}
              onMouseEnter={(e) => {
                if (currentPage !== item.id) {
                  e.currentTarget.style.backgroundColor = "var(--glass)";
                  e.currentTarget.style.borderColor = "var(--border)";
                }
              }}
              onMouseLeave={(e) => {
                if (currentPage !== item.id) {
                  e.currentTarget.style.backgroundColor = "transparent";
                  e.currentTarget.style.borderColor = "transparent";
                }
              }}
            >
              <span style={{ fontSize: "18px", marginRight: sidebarOpen ? "0.75rem" : "0" }}>
                {item.icon}
              </span>
              {sidebarOpen && (
                <span style={{ fontSize: "14px" }}>
                  {item.label}
                </span>
              )}
            </div>
          ))}
        </nav>
      </div>

      {/* Main Content */}
      <div style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
      }}>
        {/* Header */}
        <header style={{
          backgroundColor: "#2d2d2d",
          borderBottom: "1px solid #404040",
          padding: "0.75rem 1.5rem",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}>
          <div>
            <h1 style={{
              color: "#ffffff",
              margin: 0,
              fontSize: "24px",
            }}>
              XFChess Tournament Admin
            </h1>
            <p style={{
              color: "#999",
              margin: "0.25rem 0 0 0",
              fontSize: "14px",
            }}>
              {menuItems.find(item => item.id === currentPage)?.label || "Dashboard"}
            </p>
          </div>

          <div style={{
            display: "flex",
            alignItems: "center",
            gap: "1rem",
          }}>
            <div style={{
              display: "flex",
              alignItems: "center",
              gap: "0.5rem",
              padding: "0.5rem 1rem",
              backgroundColor: "#404040",
              borderRadius: "4px",
            }}>
              <div style={{
                width: "8px",
                height: "8px",
                borderRadius: "50%",
                backgroundColor: "#4ade80",
              }} />
              <span style={{
                fontSize: "12px",
                color: "#999",
              }}>
                Connected
              </span>
            </div>

            <div style={{
              display: "flex",
              alignItems: "center",
              gap: "0.5rem",
            }}>
              <span style={{
                fontSize: "14px",
                color: "#999",
              }}>
                Admin
              </span>
              <button
                onClick={logout}
                style={{
                  backgroundColor: "#ef4444",
                  color: "#ffffff",
                  border: "none",
                  borderRadius: "4px",
                  padding: "0.5rem 1rem",
                  cursor: "pointer",
                  fontSize: "14px",
                  transition: "background-color 0.2s ease",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = "#dc2626";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = "#ef4444";
                }}
              >
                Logout
              </button>
            </div>
          </div>
        </header>

        {/* Page Content */}
        <main style={{
          flex: 1,
          padding: "1.25rem 1.5rem",
          overflow: "auto",
        }}>
          {children}
        </main>
      </div>
    </div>
  );
}

