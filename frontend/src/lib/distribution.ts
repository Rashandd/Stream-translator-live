export type IntegrationStatus = {
  name: string;
  configured: boolean;
  plans: string;
  enabled_for_plan: boolean;
  connected: boolean;
};

export type IntegrationUiState =
  | "loading"
  | "unknown"
  | "not_configured"
  | "plan_locked"
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
  if (!status.enabled_for_plan) return "plan_locked";
  if (status.connected) return "connected";
  return "available";
}

export function parsePlanCsv(plans: string): string[] {
  return plans
    .split(",")
    .map((plan) => plan.trim())
    .filter(Boolean);
}

export function serializePlanCsv(plans: Iterable<string>): string {
  return [...plans].map((plan) => plan.trim()).filter(Boolean).sort().join(",");
}
