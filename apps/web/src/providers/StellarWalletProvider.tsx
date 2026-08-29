"use client";

import React, {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  useRef,
} from "react";
import {
  StellarWalletsKit,
  WalletNetwork,
  allowAllModules,
} from "@creit.tech/stellar-wallets-kit";
import { AlertCircle } from "lucide-react";

import { safeGetItem, safeSetItem, safeRemoveItem, isStorageAvailable } from "@/utils/safe-storage";
import { isValidStellarAddress } from "@/utils/stellar-validation";

import { offrampService } from "@/services/offramp.service";
import { notify } from "@/utils/notification";
import { NETWORK_PASSPHRASE } from "@/lib/constants";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export type WalletId = string;
export type ConnectionStatus = "idle" | "connecting" | "connected" | "disconnecting";

interface WalletContextType {
  connect: (walletId: WalletId) => Promise<void>;
  disconnect: () => Promise<void>;
  address: string | null;
  isConnected: boolean;
  isConnecting: boolean;
  connectionStatus: ConnectionStatus;
  selectedWalletId: string | null;
  network: WalletNetwork;
  setNetwork: (network: WalletNetwork) => Promise<void>;
  signTransaction: (xdr: string) => Promise<string>;
  openModal: () => void;
  closeModal: () => void;
  isModalOpen: boolean;
  supportedWallets: { id: WalletId; name: string; icon: string }[];
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export const useWallet = () => {
  const context = useContext(WalletContext);
  if (!context) {
    throw new Error("useWallet must be used within a StellarWalletProvider");
  }
  return context;
};

/** Derive the human-readable name for a given network passphrase. */
function getNetworkName(passphrase: string): string {
  switch (passphrase) {
    case WalletNetwork.PUBLIC:
      return "Mainnet";
    case WalletNetwork.TESTNET:
      return "Testnet";
    case WalletNetwork.FUTURENET:
      return "Futurenet";
    default:
      return "Unknown Network";
  }
}

export const StellarWalletProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const [address, setAddress] = useState<string | null>(() => {
    if (typeof window === 'undefined') return null;
    const savedAddress = safeGetItem("stellar_wallet_address");
    const savedNetwork = safeGetItem("stellar_wallet_network");
    console.log('Lazy init address:', { savedAddress, savedNetwork });
    if (savedNetwork === WalletNetwork.TESTNET && savedAddress && isValidStellarAddress(savedAddress)) {
      return savedAddress;
    }
    return null;
  });
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>(() => {
    if (typeof window === 'undefined') return "idle";
    const savedAddress = safeGetItem("stellar_wallet_address");
    const savedWalletId = safeGetItem("stellar_wallet_id");
    const savedNetwork = safeGetItem("stellar_wallet_network");
    console.log('Lazy init connectionStatus:', { savedAddress, savedWalletId, savedNetwork });
    if (savedAddress && isValidStellarAddress(savedAddress) && savedWalletId && savedNetwork === WalletNetwork.TESTNET) {
      return "connected";
    }
    return "idle";
  });
  const [selectedWalletId, setSelectedWalletId] = useState<WalletId | null>(() => {
    if (typeof window === 'undefined') return null;
    const savedAddress = safeGetItem("stellar_wallet_address");
    const savedWalletId = safeGetItem("stellar_wallet_id");
    const savedNetwork = safeGetItem("stellar_wallet_network");
    console.log('Lazy init selectedWalletId:', { savedWalletId, savedNetwork });
    if (savedNetwork === WalletNetwork.TESTNET && savedAddress && isValidStellarAddress(savedAddress)) {
      return savedWalletId as WalletId | null;
    }
    return null;
  });
  const [network, setNetworkState] = useState<WalletNetwork>(WalletNetwork.TESTNET);
  const [kit, setKit] = useState<StellarWalletsKit | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isPersistenceAvailable, setIsPersistenceAvailable] = useState(true);

  // Network mismatch modal state
  const [networkMismatchOpen, setNetworkMismatchOpen] = useState(false);
  const [mismatchInfo, setMismatchInfo] = useState<{
    walletNetwork: string;
    expectedNetwork: string;
  } | null>(null);

  // Holds the AbortController for the current in-flight connection attempt.
  // Aborting it signals connect() to discard any resolved address.
  const connectionAbortRef = useRef<AbortController | null>(null);

  // Initialize kit and handle persistence
  useEffect(() => {
    setIsPersistenceAvailable(isStorageAvailable());

    const walletKit = new StellarWalletsKit({
      network: network,
      modules: allowAllModules(),
    });
    setKit(walletKit);

    // RESTORE SESSION
    const savedAddress = safeGetItem("stellar_wallet_address");
    const savedWalletId = safeGetItem("stellar_wallet_id");
    const savedNetwork = safeGetItem("stellar_wallet_network");

    if (savedAddress && savedWalletId && savedNetwork === network) {
      if (!isValidStellarAddress(savedAddress)) {
        // Tampered or invalid address — clear storage and force reconnect
        safeRemoveItem("stellar_wallet_address");
        safeRemoveItem("stellar_wallet_id");
        safeRemoveItem("stellar_wallet_network");
        setAddress(null);
        setSelectedWalletId(null);
        setConnectionStatus("idle");
        return;
      }
      walletKit.setWallet(savedWalletId);

      // Sync with backend on session restoration
      offrampService.syncWallet(savedAddress);
    }
  }, [network]);

  const disconnect = useCallback(async () => {
    // Abort any in-flight connection so its result is discarded
    if (connectionAbortRef.current) {
      connectionAbortRef.current.abort();
      connectionAbortRef.current = null;
    }

    setConnectionStatus("disconnecting");
    setAddress(null);
    setSelectedWalletId(null);
    safeRemoveItem("stellar_wallet_address");
    safeRemoveItem("stellar_wallet_id");
    safeRemoveItem("stellar_wallet_network");
    setConnectionStatus("idle");
  }, []);

  const setNetwork = useCallback(
    async (newNetwork: WalletNetwork) => {
      if (newNetwork === network) return;

      // Block network switch while a connection is in progress — abort it first
      if (connectionAbortRef.current) {
        connectionAbortRef.current.abort();
        connectionAbortRef.current = null;
      }

      // Fully await disconnect so state is clean before the network changes
      await disconnect();
      setNetworkState(newNetwork);
    },
    [network, disconnect],
  );

  const supportedWallets: { id: WalletId; name: string; icon: string }[] = [
    { id: "freighter", name: "Freighter", icon: "/icons/freighter.png" },
    { id: "albedo", name: "Albedo", icon: "/icons/albedo.png" },
    { id: "xbull", name: "xBull", icon: "/icons/xbull.png" },
    { id: "rabet", name: "Rabet", icon: "/icons/rabet.png" },
    { id: "lobstr", name: "Lobstr", icon: "/icons/lobstr.png" },
  ];

  const WALLET_INSTALL_URL: Partial<Record<WalletId, string>> = {
    freighter: "https://freighter.app/",
    xbull: "https://xbull.app/",
    rabet: "https://rabet.io/",
    albedo: "https://albedo.link/",
    lobstr: "https://lobstr.co/",
  };

  const connect = useCallback(async (walletId: WalletId) => {
    if (!kit) return;

    // Abort any previous in-flight attempt before starting a new one
    if (connectionAbortRef.current) {
      connectionAbortRef.current.abort();
    }

    const controller = new AbortController();
    connectionAbortRef.current = controller;
    const { signal } = controller;

    try {
      kit.setWallet(walletId);
      setConnectionStatus("connecting");
      setIsModalOpen(false);

      let timeoutId: ReturnType<typeof setTimeout>;
      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          controller.abort();
          reject(new Error("Connection attempt timed out after 30 seconds"));
        }, 30000);
      });

      // Await the potentially long-running wallet handshake
      const response = await Promise.race([
        kit.getAddress(),
        timeoutPromise
      ]);

      clearTimeout(timeoutId!);

      // If disconnect() or setNetwork() was called while we were awaiting,
      // the signal is aborted — discard this result entirely.
      if (signal.aborted) return;

      const { address: resolvedAddress } = response as { address: string };

      if (!resolvedAddress) {
        throw new Error(
          "No address returned from wallet. Please ensure your wallet is unlocked and try again.",
        );
      }

      // ── Network passphrase mismatch detection ──────────────────────────────
      // The app targets a specific network (e.g. Testnet). If the wallet is
      // configured for a different network (e.g. Mainnet), any transaction it
      // signs will be invalid on the app's target network. Detect this early
      // and refuse the connection, prompting the user to switch.
      try {
        const { networkPassphrase: walletPassphrase } = await kit.getNetwork();

        if (signal.aborted) return;

        // The expected passphrase comes from env config (NEXT_PUBLIC_NETWORK_PASSPHRASE)
        // falling back to the WalletNetwork enum value for the app's configured network.
        const expectedPassphrase = NETWORK_PASSPHRASE ?? network;

        if (walletPassphrase !== expectedPassphrase) {
          const walletNetworkName = getNetworkName(walletPassphrase);
          const expectedNetworkName = getNetworkName(expectedPassphrase);

          // Surface the mismatch modal to the user
          setMismatchInfo({
            walletNetwork: walletNetworkName,
            expectedNetwork: expectedNetworkName,
          });
          setNetworkMismatchOpen(true);

          // Reset connection state — do not store the address
          setConnectionStatus("idle");
          return;
        }
      } catch (networkError) {
        // getNetwork() is not supported by all wallets (e.g. hardware wallets,
        // older extensions). Log the failure but allow the connection to proceed
        // rather than blocking users with unsupported wallets.
        console.warn(
          "[StellarWalletProvider] Could not verify wallet network passphrase:",
          networkError,
        );
      }
      // ── End network passphrase mismatch detection ──────────────────────────

      setAddress(resolvedAddress);
      setSelectedWalletId(walletId);
      setConnectionStatus("connected");
      safeSetItem("stellar_wallet_address", resolvedAddress);
      safeSetItem("stellar_wallet_id", walletId);
      safeSetItem("stellar_wallet_network", network);

      // Sync with backend on new connection
      offrampService.syncWallet(resolvedAddress);
    } catch (error: unknown) {
      // Don't surface errors for intentionally aborted connections (except timeouts)
      if (signal.aborted && !(error instanceof Error && error.message.includes("timed out"))) return;

      let errorMessage = "Unknown connection error";
      if (error instanceof Error) errorMessage = error.message;
      else if (typeof error === "string") errorMessage = error;
      else if (error && typeof error === "object" && "message" in error)
        errorMessage = String((error as { message: unknown }).message);

      if (errorMessage.toLowerCase().includes("not installed")) {
        const installHref = WALLET_INSTALL_URL[walletId];

        notify.error(
          <div className="flex flex-col gap-1">
            <span>{walletId} wallet extension is not detected.</span>
            {installHref ? (
              <a
                href={installHref}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-violet-400 hover:text-violet-300 transition-colors underline underline-offset-2"
              >
                Install / get wallet
              </a>
            ) : (
              <span className="text-xs text-white/70">
                Install the wallet extension (or enable it) and try again.
              </span>
            )}
          </div>,
        );
      } else if (
        errorMessage.toLowerCase().includes("user rejected") ||
        errorMessage.toLowerCase().includes("permission denied")
      ) {
        notify.error("Connection rejected by user");
      } else {
        // Show a generic but helpful error for other errors
        notify.error(`Failed to connect to ${walletId}: ${errorMessage}`);
      }

      setConnectionStatus("idle");
    } finally {
      // Only clear the ref if this controller is still the active one
      if (connectionAbortRef.current === controller) {
        connectionAbortRef.current = null;
      }
    }
  }, [kit, network]);

  const signTransaction = useCallback(
    async (xdr: string) => {
      if (!kit || !address) throw new Error("Wallet not connected");
      try {
        const { signedTxXdr } = await kit.signTransaction(xdr);
        return signedTxXdr;
      } catch (error) {
        throw error;
      }
    },
    [kit, address],
  );

  const openModal = useCallback(() => setIsModalOpen(true), []);
  const closeModal = useCallback(() => setIsModalOpen(false), []);

  return (
    <WalletContext.Provider
      value={{
        connect,
        disconnect,
        address,
        isConnected: connectionStatus === "connected",
        isConnecting: connectionStatus === "connecting",
        connectionStatus,
        selectedWalletId,
        network,
        setNetwork,
        signTransaction,
        openModal,
        closeModal,
        isModalOpen,
        supportedWallets,
      }}
    >
      {children}

      {/* ── Network Mismatch Modal ─────────────────────────────────────────── */}
      <Dialog
        open={networkMismatchOpen}
        onOpenChange={(open) => {
          if (!open) setNetworkMismatchOpen(false);
        }}
      >
        <DialogContent
          className="max-w-md border-white/10 bg-[#0F1621]"
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-white">
              <AlertCircle className="h-5 w-5 text-yellow-400" aria-hidden="true" />
              Wrong Network Detected
            </DialogTitle>
            <DialogDescription className="text-[#92A5A8]">
              {mismatchInfo ? (
                <>
                  Your wallet is set to{" "}
                  <span className="font-semibold text-white">
                    {mismatchInfo.walletNetwork}
                  </span>
                  , but this app targets{" "}
                  <span className="font-semibold text-white">
                    {mismatchInfo.expectedNetwork}
                  </span>
                  . Submitting transactions on the wrong network will fail.
                </>
              ) : (
                "Your wallet network does not match the app's expected network."
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="rounded-lg border border-yellow-500/20 bg-yellow-500/5 px-4 py-3 text-sm text-yellow-300">
            Please switch your wallet to{" "}
            <span className="font-semibold">
              {mismatchInfo?.expectedNetwork ?? "the correct network"}
            </span>{" "}
            and try connecting again.
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              className="border-white/10 text-white hover:bg-white/10"
              onClick={() => setNetworkMismatchOpen(false)}
            >
              Dismiss
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {/* ── End Network Mismatch Modal ─────────────────────────────────────── */}

      {!isPersistenceAvailable && (
        <div className="fixed bottom-4 right-4 z-50 px-3 py-2 bg-yellow-500/10 border border-yellow-500/20 text-yellow-500 text-xs rounded-md shadow-lg flex items-center gap-2">
          <AlertCircle className="w-4 h-4" />
          <span>Private browsing mode: Wallet connection will not be saved.</span>
        </div>
      )}
    </WalletContext.Provider>
  );
};
