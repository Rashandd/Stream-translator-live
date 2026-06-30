import publicConfig from "@/src/config/public-config.json";

export type PublicConfig = typeof publicConfig;

export async function getPublicConfig(): Promise<PublicConfig> {
  return publicConfig;
}
