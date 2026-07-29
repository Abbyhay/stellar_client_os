"use client";
import { motion } from "framer-motion";
import { Wallet, Check } from "lucide-react";
import React from "react";
import { useWallet, WalletId } from "@/providers/StellarWalletProvider";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

export function WalletModal() {
  const { isModalOpen, closeModal, supportedWallets, connect, isConnecting, isConnected } =
    useWallet();

  const [activeSelection, setActiveSelection] = React.useState<WalletId | null>(
    null,
  );

  // Auto-close when connected
  React.useEffect(() => {
    if (isConnected && isModalOpen) {
      closeModal();
    }
  }, [isConnected, isModalOpen, closeModal]);

  // Reset selection when modal opens
  React.useEffect(() => {
    if (isModalOpen) setActiveSelection(null);
  }, [isModalOpen]);

  const handleConnectClick = async () => {
    if (activeSelection) {
      await connect(activeSelection);
    }
  };

  // Radix restores focus to whichever element opened the dialog. After a
  // successful connect that trigger unmounts (ConnectButton swaps to its
  // connected state), so the default restore targets a detached node and focus
  // falls back to <body> — the keyboard user is dropped outside the app's tab
  // order. Redirect to whichever wallet trigger is currently mounted instead.
  const handleCloseAutoFocus = React.useCallback((event: Event) => {
    const walletTrigger = document.querySelector<HTMLElement>(
      "[data-wallet-trigger]",
    );
    if (walletTrigger) {
      event.preventDefault();
      walletTrigger.focus();
    }
  }, []);

  return (
    <Dialog open={isModalOpen} onOpenChange={(open) => !open && closeModal()}>
      <DialogContent
        onCloseAutoFocus={handleCloseAutoFocus}
        className="max-w-md p-1 overflow-hidden border-white/10 bg-[#0F1621] rounded-3xl shadow-2xl"
      >
        {/* Glossy overlay effect */}
        <div
          aria-hidden="true"
          className="absolute inset-0 bg-linear-to-br from-white/5 to-transparent pointer-events-none"
        />

        <div className="relative bg-[#0F1621] rounded-[22px] p-8 flex flex-col">
          {/* Header */}
          <DialogHeader className="mb-8 items-start justify-between flex-row">
            <div>
              <DialogTitle className="text-2xl font-bold text-white tracking-tight">
                Connect Wallet
              </DialogTitle>
              <DialogDescription className="mt-1 text-[#92A5A8] text-sm">
                Select your preferred Stellar wallet
              </DialogDescription>
            </div>
          </DialogHeader>

          {/* Wallet List */}
          <div className="flex flex-col gap-3 mb-8">
            {supportedWallets.map((wallet) => {
              const isSelected = activeSelection === wallet.id;
              return (
                <button
                  key={wallet.id}
                  type="button"
                  aria-pressed={isSelected}
                  onClick={() => setActiveSelection(wallet.id)}
                  className={`group relative flex items-center gap-4 w-full p-4 rounded-2xl transition-all border ${isSelected
                    ? "bg-white/10 border-white/30 shadow-[0_0_20px_rgba(255,255,255,0.05)]"
                    : "bg-white/5 border-white/5 hover:bg-white/10 hover:border-white/10"
                    }`}
                >
                  {/* Selection indicator */}
                  <div
                    className={`w-6 h-6 rounded-full border-2 flex items-center justify-center transition-all duration-300 ${isSelected
                      ? "border-white bg-white"
                      : "border-white/20 bg-transparent"
                      }`}
                  >
                    {isSelected && (
                      <Check
                        className="w-4 h-4 text-[#0F1621]"
                        strokeWidth={4}
                      />
                    )}
                  </div>

                  {/* Icon */}
                  <div
                    className={`p-2 rounded-xl transition-colors ${isSelected ? "bg-white/20" : "bg-white/5"
                      }`}
                  >
                    <Wallet
                      className={`w-5 h-5 ${isSelected ? "text-white" : "text-[#92A5A8]"
                        }`}
                    />
                  </div>

                  <span
                    className={`font-semibold text-sm tracking-wide ${isSelected ? "text-white" : "text-[#92A5A8]"
                      }`}
                  >
                    {wallet.name}
                  </span>

                  {/* Hover subtle glow */}
                  <div className="absolute inset-0 rounded-2xl bg-white/0 group-hover:bg-white/5 transition-colors pointer-events-none" />
                </button>
              );
            })}
          </div>

          {/* Action Button */}
          <button
            type="button"
            onClick={handleConnectClick}
            disabled={!activeSelection || isConnecting}
            className={`group relative w-full py-4 rounded-2xl font-bold text-sm tracking-widest uppercase transition-all flex items-center justify-center gap-3 overflow-hidden shadow-lg ${activeSelection
              ? "bg-white text-[#0F1621] hover:scale-[1.02] active:scale-[0.98]"
              : "bg-white/5 text-white/20 cursor-not-allowed"
              }`}
          >
            {isConnecting ? (
              <div className="w-5 h-5 border-2 border-[#0F1621]/30 border-t-[#0F1621] rounded-full animate-spin" />
            ) : (
              <>
                <span>Connect Now</span>
                <motion.div
                  animate={{ x: [0, 4, 0] }}
                  transition={{ repeat: Infinity, duration: 1.5 }}
                >
                  <Check className="w-4 h-4" />
                </motion.div>
              </>
            )}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
