import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Markdown } from "./markdown";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

describe("Markdown", () => {
  it("renders bold and inline code as elements, not raw syntax", () => {
    const { container } = render(<Markdown text="**bold** and `code`" />);
    expect(container.querySelector("strong")?.textContent).toBe("bold");
    expect(container.querySelector("code")?.textContent).toBe("code");
    expect(container.textContent).not.toContain("**");
  });

  it("renders headings and lists", () => {
    render(<Markdown text={"# Title\n\n- one\n- two"} />);
    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
  });

  it("renders GFM tables", () => {
    render(<Markdown text={"| a | b |\n| - | - |\n| 1 | 2 |"} />);
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "a" })).toBeInTheDocument();
  });

  it("renders links as anchors", () => {
    render(<Markdown text="[example](https://example.com)" />);
    const link = screen.getByRole("link", { name: "example" });
    expect(link).toHaveAttribute("href", "https://example.com");
  });
});
