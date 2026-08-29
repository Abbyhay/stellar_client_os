import { PlanterClient, PlanterInfo, ReferralInfo } from "@fundable/sdk";

/**
 * Service for interacting with the Planter referral system.
 */
export class SocialService {
  private planterClient: PlanterClient | null = null;

  /**
   * Initialize the social service with the planter contract client.
   * @param contractId The deployed planter contract ID
   * @param networkPassphrase The network passphrase
   * @param rpcUrl The RPC URL for Soroban
   */
  initialize(
    contractId: string,
    networkPassphrase: string,
    rpcUrl: string
  ) {
    this.planterClient = new PlanterClient({
      contractId,
      networkPassphrase,
      rpcUrl,
    });
  }

  /**
   * Register a new planter with an optional referrer.
   * @param planterAddress The planter's address
   * @param referrerAddress Optional referrer's address
   */
  async registerPlanter(
    planterAddress: string,
    referrerAddress?: string
  ): Promise<void> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    const tx = await this.planterClient.registerPlanter({
      planter: planterAddress,
      referrer: referrerAddress,
    });

    // Sign and send the transaction (implementation depends on wallet integration)
    // This is a placeholder - actual signing would be done by the wallet
    await tx.signAndSend();
  }

  /**
   * Record a job completion for a planter.
   * @param planterAddress The planter's address
   */
  async completeJob(planterAddress: string): Promise<void> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    const tx = await this.planterClient.completeJob({
      planter: planterAddress,
    });

    await tx.signAndSend();
  }

  /**
   * Claim referral reward for a referred planter's first job completion.
   * @param referrerAddress The referrer's address
   * @param referredPlanterAddress The referred planter's address
   */
  async claimReferralReward(
    referrerAddress: string,
    referredPlanterAddress: string
  ): Promise<void> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    const tx = await this.planterClient.claimReferralReward({
      referrer: referrerAddress,
      referredPlanter: referredPlanterAddress,
    });

    await tx.signAndSend();
  }

  /**
   * Get planter information.
   * @param planterAddress The planter's address
   * @returns Planter information
   */
  async getPlanter(planterAddress: string): Promise<PlanterInfo> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    return await this.planterClient.getPlanter({
      planter: planterAddress,
    });
  }

  /**
   * Get referral information for a referrer.
   * @param referrerAddress The referrer's address
   * @returns Referral information
   */
  async getReferralInfo(referrerAddress: string): Promise<ReferralInfo> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    return await this.planterClient.getReferralInfo({
      referrer: referrerAddress,
    });
  }

  /**
   * Get current reward amount.
   * @returns Current reward amount in stroops
   */
  async getRewardAmount(): Promise<bigint> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    return await this.planterClient.getRewardAmount();
  }

  /**
   * Update reward amount (admin only).
   * @param newAmount New reward amount in stroops
   */
  async setRewardAmount(newAmount: bigint): Promise<void> {
    if (!this.planterClient) {
      throw new Error("SocialService not initialized");
    }

    const tx = await this.planterClient.setRewardAmount({
      newAmount,
    });

    await tx.signAndSend();
  }
}

// Export singleton instance
export const socialService = new SocialService();
