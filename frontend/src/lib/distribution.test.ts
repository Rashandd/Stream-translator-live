import { describe, expect, it } from "vitest";
import {
  findIntegration,
  integrationUiState,
  type IntegrationStatus,
} from "@/src/lib/distribution";

const streamlabs: IntegrationStatus = {
  name: "streamlabs",
  configured: true,
  connected: false,
};

describe("distribution integration helpers", () => {
  it("finds providers by backend name", () => {
    expect(findIntegration([streamlabs], "streamlabs")).toEqual(streamlabs);
    expect(findIntegration([streamlabs], "youtube")).toBeNull();
  });

  it("maps backend status into dashboard UI states", () => {
    expect(integrationUiState(null, true)).toBe("loading");
    expect(integrationUiState(null, false)).toBe("unknown");
    expect(integrationUiState({ ...streamlabs, configured: false }, false)).toBe("not_configured");
    expect(integrationUiState({ ...streamlabs, connected: true }, false)).toBe("connected");
    expect(integrationUiState(streamlabs, false)).toBe("available");
  });
});
