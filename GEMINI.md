Voici le contenu complet et formaté pour le fichier `gemini.md`. Tu peux copier ce bloc entier.

# Project Context & Guidelines for Gemini

This file serves as the primary context and instruction set for the AI Agent (Gemini) working on the **FastMC** project.

## 1. Project Overview
**Goal:** Develop a general-purpose, high-performance Minecraft launcher focused on modding and customization, offering the flexibility of MultiMC with a modern UI.

**Tech Stack:**
- **Language:** Rust (2021 Edition)
- **UI Framework:** Iced (Latest stable)
- **Architecture:** Modular workspace with multiple crates.

## 2. Agent Persona & Protocols
**Role:** Act as a Senior Rust Developer specialized in GUI application architecture and systems programming.

**Mandatory Pre-Commit Checks:**
Before finalizing any code generation, verify that the solution passes:
```bash
cargo fmt
cargo check
cargo clippy --fix

```

## 3. Coding Standards & Conventions

### General Rust Guidelines

* **Safety:** Strictly avoid `unsafe` Rust unless absolutely necessary and explicitly justified.
* **Style:** Adhere strictly to `rustfmt` standards.
* **Naming:**
* Files and variables: `snake_case` (e.g., `java_manager.rs`, `launch_settings`).
* Structs, Enums, Traits: `PascalCase` (e.g., `FastmcConfig`, `AccountStore`).


* **Documentation:** Add doc comments (`///`) for complex public functions and logic.

### Iced Framework Guidelines

* **Architecture:** Follow The Elm Architecture (Model-View-Update).
* **Components:** Use functional component patterns and hooks where applicable.
* **Structure:** Keep UI code clean; separate logic into `update` and layout into `view`. Use helper functions for repetitive widget composition.

### Microsoft Auth Crate Specifics

* **Purpose:** Library crate strictly for Xbox/Microsoft authentication.
* **Boundary:** Should expose simple public APIs for the main app to retrieve Xbox Live tokens.

## 4. Project Structure

Use this map to navigate the codebase and understand module relationships.

* `src/` (**App Entrypoint: fastmc**)
* `main.rs`: App shell, routing logic, stage gating, and global subscription wiring.
* `screens/`: UI screens implementing the `iced` view logic.
* `account.rs`: Offline + Microsoft device-code auth, account list, activation/deletion.
* `java_manager.rs`: Java detection, memory sliders, JVM args, status banners.
* `play.rs`: (Temp) Counter demo. **Target for replacement.**
* `server.rs`, `modpacks.rs`, `settings.rs`: Placeholders awaiting implementation.
* `mod.rs`: Screen re-exports.


* `theme.rs`: Shared styling, icon loader, and sidebar button configurations.


* `assets/`: Static assets (do not modify directly).
* `svg/`: Navigation icons for sidebar entries.


* `account_manager/` (**Crate**): Handles account persistence, offline creation, Microsoft device-code login, keyring-backed token storage, and Minecraft session retrieval (`src/lib.rs`).
* `config/` (**Crate: fastmc_config**): Manages `FastmcConfig` struct (profiles, Java, accounts), versioning/migration, and JSON persistence (`src/lib.rs`).
* `java_manager/` (**Crate**): Handles Java discovery and logic.
* `detection.rs`: Scans system paths for Java binaries.
* `settings.rs`: `JavaLaunchSettings` struct and sync helpers.
* `lib.rs`: Public exports.


* `launcher/` (**Crate**): Builds the CLI command for Vanilla Minecraft (auth, memory, natives, classpath, game args).
* `microsoft_auth/` (**Crate**): Microsoft OAuth device flow implementation.
* `authenticator.rs`: Device-code polling logic.
* `errors.rs`: Auth-specific error handling.
* `models.rs`: DTOs for tokens and device codes.



## 5. Development Status Snapshot

### Completed Functionality

* **Shell:** Functional sidebar navigation and stage gating (requires login to proceed).
* **Accounts:** Full support for Offline & Microsoft (device-code) auth. Persistence via `AccountStore`. Active account selection/deletion works.
* **Java Manager:** Detection of system Java, custom path support, memory sliders, JVM args. Settings persist to `FastmcConfig`.
* **Theme:** SVG icon loading and shared sidebar styling.

### Pending / Placeholders

* **Play Screen:** Currently a counter demo. Needs full launch flow.
* **Screens:** Server, Modpacks, and Settings are static placeholders.

## 6. Immediate Objectives (Roadmap)

1. **Implement Play Screen:** Replace the placeholder. Implement version/profile selection, download/install progress UI, and launch logic using the `launcher` crate.
2. **Build Server Screen:** Create CRUD interface for server entries and "Quick Join" functionality.
3. **Implement Modpacks Screen:** Add provider selection (local/remote catalogs), browsing, installation, and update flows.
4. **Expand Settings:** Global options (theme, cache, telemetry) synced with `FastmcConfig`.
5. **Refine Auth:** Improve device-code polling UX (clearer states) and token refresh logic.
6. **Enhance Validation:** Add checks for executable permissions in Java Manager and strict schema validation for config.

---

**Instruction for Gemini:** When generating code, ensure you provide the complete file content or clearly marked diffs. Explain architectural decisions if significant changes are made to the `iced` message passing flow.
