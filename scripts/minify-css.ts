import browserslist from "browserslist";
import init, { browserslistToTargets, transform } from "lightningcss";

await init();

const targets = browserslistToTargets(
    browserslist(
        "> 0.5%, last 2 versions, Firefox >= 115, Firefox ESR, not dead",
    ),
);

async function minifyStylesheet(path: string) {
    try {
        const inputPath = `./assets/${path}`;
        const outputPath = `./public/${path}`;
        const css = await Deno.readTextFile(inputPath);
        const { code: outputCss } = transform({
            filename: inputPath,
            code: new TextEncoder().encode(css),
            minify: true,
            targets,
        });

        const decoder = new TextDecoder();
        await Deno.writeTextFile(outputPath, `${decoder.decode(outputCss)}\n`);
    } catch (error: unknown) {
        console.error(
            `Error building styles for path ${path}: ${error as string}`,
        );
    }
}

const cssFiles = ["fonts/fonts.css", "static/css/index.css"];

for await (const file of cssFiles) {
    minifyStylesheet(file);
}
