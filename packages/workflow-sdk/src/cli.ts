#!/usr/bin/env node
/**
 * CLI tool for @boternity/workflow-sdk.
 *
 * Commands:
 *   build <file>     Compile a .workflow.ts file to .yaml
 *   validate <file>  Validate a .workflow.ts file without writing output
 *
 * Usage:
 *   workflow-sdk build my-workflow.workflow.ts
 *   workflow-sdk build my-workflow.workflow.ts -o output.yaml
 *   workflow-sdk validate my-workflow.workflow.ts
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve, basename } from "node:path";
import { pathToFileURL } from "node:url";

interface CliArgs {
  command: "build" | "validate" | "help";
  file?: string;
  output?: string;
}

function parseArgs(args: string[]): CliArgs {
  if (args.length === 0 || args[0] === "help" || args[0] === "--help" || args[0] === "-h") {
    return { command: "help" };
  }

  const command = args[0];
  if (command !== "build" && command !== "validate") {
    console.error(`Unknown command: ${command}`);
    return { command: "help" };
  }

  const file = args[1];
  if (!file) {
    console.error(`Missing file argument for '${command}' command`);
    return { command: "help" };
  }

  let output: string | undefined;
  const outputIdx = args.indexOf("-o");
  if (outputIdx !== -1 && args[outputIdx + 1]) {
    output = args[outputIdx + 1];
  }

  return { command, file, output };
}

function printHelp(): void {
  console.log(`@boternity/workflow-sdk CLI

Usage:
  workflow-sdk build <file.workflow.ts>             Build .workflow.ts to .yaml
  workflow-sdk build <file.workflow.ts> -o out.yaml Build with custom output path
  workflow-sdk validate <file.workflow.ts>          Validate without writing

Options:
  -o <path>    Output file path (default: <name>.yaml)
  -h, --help   Show this help message
`);
}

async function loadWorkflowModule(filePath: string): Promise<string> {
  const absPath = resolve(filePath);

  // Check file exists
  try {
    readFileSync(absPath);
  } catch {
    throw new Error(`File not found: ${absPath}`);
  }

  // Dynamic import of the .workflow.ts file
  // The file must export `default` as a WorkflowBuilder or WorkflowDefinition
  const fileUrl = pathToFileURL(absPath).href;
  const mod = await import(fileUrl);

  const exported = mod.default;
  if (!exported) {
    throw new Error(
      `File "${filePath}" must have a default export (WorkflowBuilder or WorkflowDefinition)`,
    );
  }

  // If it's a builder, call toYaml()
  if (typeof exported.toYaml === "function") {
    return exported.toYaml();
  }

  // If it's a plain WorkflowDefinition object, serialize it
  if (typeof exported === "object" && exported.name && exported.steps) {
    const { stringify } = await import("yaml");
    return stringify(exported, { lineWidth: 120 });
  }

  throw new Error(
    `Default export must be a WorkflowBuilder (with .toYaml()) or a WorkflowDefinition object`,
  );
}

async function main(): Promise<void> {
  const cliArgs = parseArgs(process.argv.slice(2));

  if (cliArgs.command === "help") {
    printHelp();
    process.exit(0);
  }

  const filePath = cliArgs.file!;

  try {
    const yaml = await loadWorkflowModule(filePath);

    if (cliArgs.command === "validate") {
      console.log(`Valid: ${filePath}`);
      process.exit(0);
    }

    // Build command: write YAML output
    const outputPath =
      cliArgs.output ?? filePath.replace(/\.workflow\.ts$/, ".yaml");

    writeFileSync(outputPath, yaml, "utf-8");
    console.log(`Built: ${outputPath}`);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`Error: ${message}`);
    process.exit(1);
  }
}

main();
