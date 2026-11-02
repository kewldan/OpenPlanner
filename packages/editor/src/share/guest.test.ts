import { describe, expect, test } from "bun:test";
import { fromBase64Url, guestLink, guestProject, toBase64Url } from "./guest";

describe("гостевая ссылка", () => {
  test("байты проходят через base64url без потерь", () => {
    const bytes = Uint8Array.from({ length: 70_000 }, (_, i) => (i * 31) % 256);
    const text = toBase64Url(bytes);
    expect(text).not.toMatch(/[+/=]/);
    expect(fromBase64Url(text)).toEqual(bytes);
  });

  test("проект в якоре адреса", () => {
    const link = guestLink("https://example.org/app/#old", new Uint8Array([80, 75, 3, 4]));
    expect(link).toBe("https://example.org/app/#guest=UEsDBA");
    expect(guestProject(link.slice(link.indexOf("#")))).toEqual(new Uint8Array([80, 75, 3, 4]));
    expect(guestProject("#other=1")).toBeNull();
  });
});
