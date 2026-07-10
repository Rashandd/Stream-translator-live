import sanitizeHtml from "sanitize-html";

const ALLOWED_TAGS = [
  "h1",
  "h2",
  "h3",
  "p",
  "ul",
  "ol",
  "li",
  "strong",
  "em",
  "a",
  "blockquote",
] as const;

/**
 * Defense-in-depth for admin-authored legal documents.
 *
 * The backend is the source-of-truth sanitizer at write and read boundaries.
 * Public rendering repeats the same small allowlist. The live admin preview
 * uses a fully sandboxed iframe, so even an unsaved draft cannot execute.
 */
export function sanitizeLegalHtml(value: string): string {
  return sanitizeHtml(value, {
    allowedTags: [...ALLOWED_TAGS],
    allowedAttributes: {
      a: ["href", "title", "rel"],
    },
    allowedSchemes: ["http", "https", "mailto"],
    allowProtocolRelative: false,
    disallowedTagsMode: "discard",
    enforceHtmlBoundary: true,
    transformTags: {
      a: (_tagName, attributes) => ({
        tagName: "a",
        attribs: {
          ...attributes,
          rel: "noopener noreferrer",
        },
      }),
    },
  });
}
