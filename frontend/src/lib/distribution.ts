export type IntegrationStatus = {
  name: string;
  configured: boolean;
  connected: boolean;
};

export type IntegrationUiState =
  | "loading"
  | "unknown"
  | "not_configured"
  | "available"
  | "connected";

export function findIntegration(
  items: IntegrationStatus[] | null,
  provider: string,
): IntegrationStatus | null {
  return items?.find((item) => item.name === provider) ?? null;
}

export function integrationUiState(
  status: IntegrationStatus | null,
  isLoading: boolean,
): IntegrationUiState {
  if (isLoading) return "loading";
  if (!status) return "unknown";
  if (!status.configured) return "not_configured";
  if (status.connected) return "connected";
  return "available";
}
