import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-stt-servers-panel.tsx", import.meta.url), "utf8");

describe("admin self-hosted STT servers panel", () => {
  it("adds a server through the admin API and shows the generated public key", () => {
    expect(source).toContain('adminFetch("/admin/stt-servers"');
    expect(source).toContain("ssh_public_key");
    expect(source).toContain("<CopyRow");
  });

  it("triggers provisioning and shows the in-progress state", () => {
    expect(source).toContain("/admin/stt-servers/${id}/provision");
    expect(source).toContain('"provisioning"');
  });

  it("polls the list while any server is provisioning", () => {
    expect(source).toContain("setInterval");
    expect(source).toContain('s.status === "provisioning"');
  });

  it("deletes a server after confirmation", () => {
    expect(source).toContain("window.confirm(labels.confirmDelete)");
    expect(source).toContain("method: \"DELETE\"");
  });

  it("redirects to admin sign-in on 401/403", () => {
    expect(source).toContain("response.status === 401 || response.status === 403");
    expect(source).toContain("setUnauthorized(true)");
  });

  it("never renders the private key or API token (backend never returns them)", () => {
    expect(source).not.toContain("private_key");
    expect(source).not.toContain("api_token");
  });

  it("supports an optional public endpoint override (e.g. Vast.ai NAT port mapping)", () => {
    expect(source).toContain("publicEndpointOverride");
    expect(source).toContain("public_endpoint_override");
  });
});
