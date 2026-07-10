import { describe, expect, it } from "vitest";
import { sanitizeLegalHtml } from "./legal-html";

describe("sanitizeLegalHtml", () => {
  it("keeps the legal document allowlist", () => {
    const input = [
      "<h1>Terms</h1>",
      "<p>Read <strong>carefully</strong> and <em>ask questions</em>.</p>",
      "<ol><li>First</li><li>Second</li></ol>",
      '<blockquote>Plain language</blockquote><a href="https://strovak.com/legal" title="Legal">Details</a>',
    ].join("");

    const result = sanitizeLegalHtml(input);

    expect(result).toContain("<h1>Terms</h1>");
    expect(result).toContain("<strong>carefully</strong>");
    expect(result).toContain("<em>ask questions</em>");
    expect(result).toContain("<ol><li>First</li><li>Second</li></ol>");
    expect(result).toContain("https://strovak.com/legal");
  });

  it.each([
    ["script", "<script>window.stolen = true</script><p>Safe</p>"],
    ["event handler", '<img src=x onerror="alert(1)"><p onclick=alert(2)>Safe</p>'],
    ["javascript URL", '<a href="javascript:alert(1)">Click</a>'],
    ["encoded javascript URL", '<a href="jav&#x61;script:alert(1)">Click</a>'],
    ["data URL", '<a href="data:text/html,<script>alert(1)</script>">Click</a>'],
    ["SVG", '<svg><a xlink:href="javascript:alert(1)">Click</a></svg>'],
    ["malformed markup", '<scr<script>ipt>alert(1)</scr</script>ipt><p>Safe</p>'],
  ])("removes executable %s payloads", (_name, input) => {
    const result = sanitizeLegalHtml(input).toLowerCase();

    expect(result).not.toContain("<script");
    expect(result).not.toContain("onerror");
    expect(result).not.toContain("onclick");
    expect(result).not.toContain("javascript:");
    expect(result).not.toContain("data:text/html");
    expect(result).not.toContain("<svg");
  });

  it("discards the contents of active-content elements", () => {
    const result = sanitizeLegalHtml(
      "<script>window.stolen = true</script><style>body{display:none}</style><p>Safe</p>",
    );
    expect(result).toBe("<p>Safe</p>");
  });
});
