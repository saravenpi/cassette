import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ fetch, setHeaders }) => {
  setHeaders({ "cache-control": "public, max-age=1800" });
  try {
    const res = await fetch(
      "https://api.github.com/repos/saravenpi/cassette/releases/latest",
      { headers: { accept: "application/vnd.github+json" } }
    );
    if (!res.ok) {
      return {
        version: "0.15.3",
        url: "https://github.com/saravenpi/cassette/releases/latest",
        publishedAt: null,
        assets: []
      };
    }
    const data = await res.json();
    const tag = typeof data.tag_name === "string" ? data.tag_name : "v0.15.3";
    const url = typeof data.html_url === "string" ? data.html_url : "https://github.com/saravenpi/cassette/releases/latest";
    const assets = Array.isArray(data.assets)
      ? data.assets.map((a: { name: string; size: number; browser_download_url: string }) => ({
          name: a.name,
          size: a.size,
          browser_download_url: a.browser_download_url
        }))
      : [];

    return {
      version: tag.replace(/^v/, ""),
      url,
      publishedAt: data.published_at ?? null,
      assets
    };
  } catch {
    return {
      version: "0.15.3",
      url: "https://github.com/saravenpi/cassette/releases/latest",
      publishedAt: null,
      assets: []
    };
  }
};
