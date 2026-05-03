// Extension: ghcp-mon-workflows
// Registers repo-local skills describing recurring multi-step workflows
// for the ghcp-mon repository.

import { joinSession } from "@github/copilot-sdk/extension";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const SKILLS_DIR = join(__dirname, "skills");

await joinSession({
    skillDirectories: [SKILLS_DIR],
});
