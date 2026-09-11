import { WorkspaceService } from '@/application/services/domains';
import {
  checkStructuredSpacesSupported,
  clearStructuredSpacesCapabilityCache,
  getStructuredSpacesCapability,
  recordStructuredSpacesSupported,
  recordStructuredSpacesUnsupported,
} from '@/application/services/js-services/http/spaceCapability';

const mockGetSpaces = jest.fn();

jest.mock('@/application/services/domains', () => ({
  WorkspaceService: {
    getSpaces: (...args: unknown[]) => mockGetSpaces(...args),
  },
}));

describe('spaceCapability', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    clearStructuredSpacesCapabilityCache();
    window.sessionStorage.clear();
  });

  it('returns undefined when capability is unknown', () => {
    expect(getStructuredSpacesCapability('ws-unknown')).toBeUndefined();
  });

  it('records supported status in memory and session storage', () => {
    recordStructuredSpacesSupported('ws-1');
    expect(getStructuredSpacesCapability('ws-1')).toBe(true);
    expect(window.sessionStorage.getItem('af_structured_spaces_supported:ws-1')).toBe('true');
  });

  it('records unsupported status in memory and session storage', () => {
    recordStructuredSpacesUnsupported('ws-1');
    expect(getStructuredSpacesCapability('ws-1')).toBe(false);
    expect(window.sessionStorage.getItem('af_structured_spaces_supported:ws-1')).toBe('false');
  });

  it('reads from session storage when memory cache is cleared', () => {
    window.sessionStorage.setItem('af_structured_spaces_supported:ws-saved', 'true');
    expect(getStructuredSpacesCapability('ws-saved')).toBe(true);

    window.sessionStorage.setItem('af_structured_spaces_supported:ws-legacy', 'false');
    expect(getStructuredSpacesCapability('ws-legacy')).toBe(false);
  });

  it('checks capability via getSpaces and caches true when successful', async () => {
    mockGetSpaces.mockResolvedValueOnce({ spaces: [] });

    const result = await checkStructuredSpacesSupported('ws-modern');

    expect(result).toBe(true);
    expect(mockGetSpaces).toHaveBeenCalledWith('ws-modern');
    expect(getStructuredSpacesCapability('ws-modern')).toBe(true);
  });

  it('caches false when getSpaces fails with a 404 route error', async () => {
    mockGetSpaces.mockRejectedValueOnce({
      httpStatus: 404,
      message: 'Route not found',
    });

    const result = await checkStructuredSpacesSupported('ws-legacy-server');

    expect(result).toBe(false);
    expect(mockGetSpaces).toHaveBeenCalledWith('ws-legacy-server');
    expect(getStructuredSpacesCapability('ws-legacy-server')).toBe(false);
  });

  it('deduplicates concurrent in-flight probes into a single request', async () => {
    let resolveProbe!: (value: unknown) => void;
    const probePromise = new Promise((resolve) => {
      resolveProbe = resolve;
    });

    mockGetSpaces.mockReturnValueOnce(probePromise);

    const check1 = checkStructuredSpacesSupported('ws-dedup');
    const check2 = checkStructuredSpacesSupported('ws-dedup');

    expect(mockGetSpaces).toHaveBeenCalledTimes(1);

    resolveProbe({ spaces: [] });
    const [res1, res2] = await Promise.all([check1, check2]);

    expect(res1).toBe(true);
    expect(res2).toBe(true);
  });

  it('does not permanently cache false on generic unexpected errors', async () => {
    mockGetSpaces.mockRejectedValueOnce(new Error('Network offline'));

    const result = await checkStructuredSpacesSupported('ws-offline');

    expect(result).toBe(false);
    expect(window.sessionStorage.getItem('af_structured_spaces_supported:ws-offline')).toBeNull();
  });
});
