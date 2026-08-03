/**
 * Stream validation utilities for payment stream forms
 */

import { StrKey } from "@stellar/stellar-sdk";

/**
 * Validate that a string is a valid Stellar contract ID (StrKey C... format)
 * @param contractId - The contract address to validate
 * @returns true if the address is a valid contract StrKey, false otherwise
 */
export function validateContractId(contractId: string): boolean {
  if (!contractId || typeof contractId !== "string") return false;
  try {
    return StrKey.isValidContract(contractId);
  } catch {
    return false;
  }
}

/**
 * Duration unit multipliers in seconds
 */
const DURATION_MULTIPLIERS = {
  hour: 60 * 60,
  day: 24 * 60 * 60,
  week: 7 * 24 * 60 * 60,
  month: 30 * 24 * 60 * 60,
  year: 365 * 24 * 60 * 60,
} as const;

export type DurationUnit = keyof typeof DURATION_MULTIPLIERS;

/**
 * Convert duration value and unit to seconds
 */
export function durationToSeconds(value: number, unit: DurationUnit): number {
  return value * DURATION_MULTIPLIERS[unit];
}

/**
 * Calculate stream end timestamp
 * @param startTime - Start timestamp in seconds (defaults to now)
 * @param durationValue - Duration value
 * @param durationUnit - Duration unit
 * @returns End timestamp in seconds
 */
export function calculateEndTime(
  startTime: number | null,
  durationValue: number,
  durationUnit: DurationUnit
): number {
  const start = startTime || Math.floor(Date.now() / 1000);
  const durationSeconds = durationToSeconds(durationValue, durationUnit);
  return start + durationSeconds;
}

/**
 * Validate that the stream end time is in the future
 * @param startTime - Start timestamp in seconds (defaults to now)
 * @param durationValue - Duration value
 * @param durationUnit - Duration unit
 * @returns Error message if invalid, null if valid
 */
export function validateEndTime(
  startTime: number | null,
  durationValue: string,
  durationUnit: string
): string | null {
  // Parse duration value
  const duration = parseInt(durationValue);
  
  if (isNaN(duration)) {
    return "Duration must be a valid number";
  }
  
  if (duration <= 0) {
    return "Duration must be greater than zero";
  }
  
  // Validate duration unit
  if (!isDurationUnit(durationUnit)) {
    return "Invalid duration unit";
  }
  
  // Calculate end time
  const endTime = calculateEndTime(startTime, duration, durationUnit);
  const now = Math.floor(Date.now() / 1000);
  
  // Check if end time is in the past
  if (endTime <= now) {
    return "Stream end time must be in the future";
  }
  
  // Warn if duration is very short (less than 1 minute)
  const durationSeconds = durationToSeconds(duration, durationUnit);
  if (durationSeconds < 60) {
    return "Duration is too short (minimum 1 minute recommended)";
  }
  
  return null;
}

/**
 * Type guard for duration units
 */
function isDurationUnit(unit: string): unit is DurationUnit {
  return unit in DURATION_MULTIPLIERS;
}

/**
 * Format timestamp to human-readable date string (UTC)
 * @param timestamp - Unix timestamp in seconds
 * @returns Formatted date string
 */
export function formatEndTime(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  
  return date.toLocaleString('en-US', {
    timeZone: 'UTC',
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    timeZoneName: 'short',
  });
}

/**
 * Get relative time description
 * @param timestamp - Unix timestamp in seconds
 * @returns Human-readable relative time
 */
export function getRelativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = timestamp - now;
  
  if (diff < 0) {
    return "in the past";
  }
  
  const minutes = Math.floor(diff / 60);
  const hours = Math.floor(diff / 3600);
  const days = Math.floor(diff / 86400);
  const weeks = Math.floor(diff / 604800);
  const months = Math.floor(diff / 2592000);
  const years = Math.floor(diff / 31536000);
  
  if (years > 0) return `in ${years} year${years !== 1 ? 's' : ''}`;
  if (months > 0) return `in ${months} month${months !== 1 ? 's' : ''}`;
  if (weeks > 0) return `in ${weeks} week${weeks !== 1 ? 's' : ''}`;
  if (days > 0) return `in ${days} day${days !== 1 ? 's' : ''}`;
  if (hours > 0) return `in ${hours} hour${hours !== 1 ? 's' : ''}`;
  if (minutes > 0) return `in ${minutes} minute${minutes !== 1 ? 's' : ''}`;
  
  return "in less than a minute";
}
