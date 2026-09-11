import { useEffect, useState } from 'react';

import { WorkspaceService } from '@/application/services/domains';
import { isUnsupportedRouteError } from '@/utils/errors';

const memoryCache = new Map<string, boolean>();
const inFlightProbes = new Map<string, Promise<boolean>>();

const STORAGE_PREFIX = 'af_structured_spaces_supported:';

function getSessionStorageItem(key: string): string | null {
  try {
    return typeof window !== 'undefined' && window.sessionStorage ? window.sessionStorage.getItem(key) : null;
  } catch {
    return null;
  }
}

function setSessionStorageItem(key: string, value: string): void {
  try {
    if (typeof window !== 'undefined' && window.sessionStorage) {
      window.sessionStorage.setItem(key, value);
    }
  } catch {
    // Ignore storage quota or security errors
  }
}

export function getStructuredSpacesCapability(workspaceId: string): boolean | undefined {
  if (memoryCache.has(workspaceId)) {
    return memoryCache.get(workspaceId);
  }

  const stored = getSessionStorageItem(`${STORAGE_PREFIX}${workspaceId}`);

  if (stored === 'true') {
    memoryCache.set(workspaceId, true);
    return true;
  }

  if (stored === 'false') {
    memoryCache.set(workspaceId, false);
    return false;
  }

  return undefined;
}

export function recordStructuredSpacesSupported(workspaceId: string): void {
  if (!workspaceId) return;
  memoryCache.set(workspaceId, true);
  setSessionStorageItem(`${STORAGE_PREFIX}${workspaceId}`, 'true');
}

export function recordStructuredSpacesUnsupported(workspaceId: string): void {
  if (!workspaceId) return;
  memoryCache.set(workspaceId, false);
  setSessionStorageItem(`${STORAGE_PREFIX}${workspaceId}`, 'false');
}

export function clearStructuredSpacesCapabilityCache(): void {
  memoryCache.clear();
  inFlightProbes.clear();
}

export async function checkStructuredSpacesSupported(workspaceId: string): Promise<boolean> {
  if (!workspaceId) return false;

  const cached = getStructuredSpacesCapability(workspaceId);

  if (cached !== undefined) return cached;

  const inFlight = inFlightProbes.get(workspaceId);

  if (inFlight) return inFlight;

  const probe = (async () => {
    try {
      if (typeof WorkspaceService?.getSpaces !== 'function') {
        return true;
      }

      await WorkspaceService.getSpaces(workspaceId);
      recordStructuredSpacesSupported(workspaceId);
      return true;
    } catch (error) {
      if (isUnsupportedRouteError(error)) {
        recordStructuredSpacesUnsupported(workspaceId);
        return false;
      }

      // Return false for transient/network errors without persistent caching
      return false;
    } finally {
      inFlightProbes.delete(workspaceId);
    }
  })();

  inFlightProbes.set(workspaceId, probe);
  return probe;
}

export function useStructuredSpacesCapability(workspaceId?: string): {
  isLegacyServer: boolean;
  isLoading: boolean;
} {
  const targetWorkspaceId = workspaceId ?? '';

  const initialCapability = targetWorkspaceId ? getStructuredSpacesCapability(targetWorkspaceId) : undefined;
  const [isLegacyServer, setIsLegacyServer] = useState<boolean>(initialCapability === false);
  const [isLoading, setIsLoading] = useState<boolean>(initialCapability === undefined && Boolean(targetWorkspaceId));

  useEffect(() => {
    if (!targetWorkspaceId) {
      setIsLegacyServer(true);
      setIsLoading(false);
      return;
    }

    const cached = getStructuredSpacesCapability(targetWorkspaceId);

    if (cached !== undefined) {
      setIsLegacyServer(!cached);
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    setIsLoading(true);
    void checkStructuredSpacesSupported(targetWorkspaceId).then((supported) => {
      if (cancelled) return;
      setIsLegacyServer(!supported);
      setIsLoading(false);
    });

    return () => {
      cancelled = true;
    };
  }, [targetWorkspaceId]);

  return { isLegacyServer, isLoading };
}
