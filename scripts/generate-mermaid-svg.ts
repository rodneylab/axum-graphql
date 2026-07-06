import type { Browser } from "puppeteer";

import { renderMermaid } from "@mermaid-js/mermaid-cli";
import { readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import puppeteer from "puppeteer";

import logger from "./logger.js";

interface Config {
  iconPacks: string[];
}

const CONFIG: Config = {
  iconPacks: ["@iconify-json/logos"],
};

const INPUT_MMD_PATHS = ["./images/high-level-architecture.mmd"];

async function generateSvg(
  browser: Browser,
  decoder: TextDecoder,
  mmd: string,
): Promise<string> {
  const { data } = await renderMermaid(browser, mmd, "svg", {
    iconPacks: CONFIG.iconPacks,
  });
  const svg = decoder.decode(data);

  return svg;
}

/*
 * Outputs SVG to the input path directory, changing extension to `.svg`
 */
async function processMmdFile(
  browser: Browser,
  decoder: TextDecoder,
  path: string,
): Promise<{ success: boolean; path: string; error?: string }> {
  try {
    const content = await readFile(path, "utf8");
    const svg = await generateSvg(browser, decoder, content);
    const name = basename(path, ".mmd");
    const dir = dirname(path);
    const outputPath = join(dir, `${name}.svg`);
    await writeFile(outputPath, svg);
    logger.info(`Written SVG output to ${outputPath}.`);

    return { success: true, path };
  } catch (err: unknown) {
    logger.error({ error: err }, `Failed to generate SVG output for ${path}`);
    const errorMessage = err instanceof Error ? err.message : String(err);

    return { success: true, path, error: errorMessage };
  }
}

process.on("unhandledRejection", (error) => {
  logger.fatal({ error }, "Unhandled promise rejection");

  process.exit(1);
});

async function main() {
  let browser: Browser;
  try {
    browser = await puppeteer.launch();
  } catch (err: unknown) {
    logger.error(
      { error: err },
      "Unexpected error launching puppeteer browser instance",
    );
    process.exit(1);
  }
  const decoder = new TextDecoder();
  try {
    const results = await Promise.allSettled(
      INPUT_MMD_PATHS.map(
        async (path) => await processMmdFile(browser, decoder, path),
      ),
    );

    const successes = results.filter(
      (result) => result.status === "fulfilled" && result.value.success,
    ).length;

    logger.info(
      `${successes.toFixed(0)} out of ${results.length.toFixed(0)} mermaid files successfully processed.`,
    );
  } finally {
    await browser.close();
  }
}

void main();
