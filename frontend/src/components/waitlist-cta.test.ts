import { describe, expect, it } from "vitest";
import { statusFromResponse } from "@/src/components/waitlist-cta";

describe("statusFromResponse", () => {
  it("maps 201 to success", () => {
    expect(statusFromResponse(201)).toBe("success");
  });

  it("maps 409 (duplicate email+interest) to duplicate", () => {
    expect(statusFromResponse(409)).toBe("duplicate");
  });

  it("maps 400 (malformed email/interest) to invalid", () => {
    expect(statusFromResponse(400)).toBe("invalid");
  });

  it("maps any other status (429, 500, ...) to a generic error", () => {
    expect(statusFromResponse(429)).toBe("error");
    expect(statusFromResponse(500)).toBe("error");
  });
});
