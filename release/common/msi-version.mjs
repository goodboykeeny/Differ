// Prints the Windows Installer form of the VERSION file's version.
import { msiVersion, version } from "./config.mjs";

try { console.log(msiVersion(version())); } catch (error) { console.error(error.message); process.exitCode = 1; }
