# Provider Extraction Next Steps

This plan picks up after the current stacked branches merge:

```text
provider-crate-extraction-phase-1  # goose-providers canonical model data
provider-crate-extraction-phase-2  # goose-types ThinkingEffort
provider-crate-extraction-phase-3  # goose-types pure ModelConfig data
provider-crate-extraction-phase-4  # first provider runtime trait + Goose adapter
```

## Goal

Fully move provider implementation code out of:

```text
crates/goose/src/providers
```

into:

```text
crates/goose-providers
```

while keeping runtime/application behavior in `goose` and shared provider-facing DTOs in `goose-types`.

Final desired ownership:

```text
goose ───────────────▶ goose-providers
  │                         │
  └────────▶ goose-types ◀──┘
```

Final state:

- `goose-providers` owns provider implementations, provider request/response formatting, provider errors, provider retry behavior, provider API clients, provider auth helpers, provider catalogs, declarative provider support, and canonical model data.
- `goose-types` owns provider-facing pure data types.
- `goose` owns config storage, sessions, agents, extension orchestration, scheduler, UI/server routes, command execution, app paths, downloads, and other runtime/application services.
- `goose-providers` does not depend on `goose`.
- `goose` depends on `goose-providers` and `goose-types`.
- Provider APIs should be consumed from `goose-providers`; do not add a long-term `goose::providers` compatibility re-export layer.

## Branch strategy

Continue the stacked branch pattern:

```text
main
  └─ provider-crate-extraction-phase-1
       └─ provider-crate-extraction-phase-2
            └─ provider-crate-extraction-phase-3
                 └─ provider-crate-extraction-phase-4
                      └─ provider-crate-extraction-phase-5
                           └─ provider-crate-extraction-phase-6
```

Each PR should target the previous phase branch until lower branches merge.

Prefer small PRs. A good PR should do one of:

- move one small DTO cluster
- move one independent utility module
- add one runtime trait method and convert one caller
- move one provider implementation
- do one dependency/feature cleanup pass

Avoid mixing these in the same PR unless the change is very small.

## Current state after the existing stack

After phases 1-4 land:

- `goose-providers` exists and owns canonical model data and lookup.
- `goose-types` exists and owns `ThinkingEffort` and pure `ModelConfig` data.
- Goose-specific `ModelConfig` construction/defaulting remains in `goose`.
- A minimal `ProviderRuntime` trait exists in `goose-providers`.
- `goose` has a thin runtime adapter over existing config behavior.
- One provider, Avian, proves the runtime adapter pattern.

The remaining provider implementation code still lives mostly in `goose`, and most provider modules still depend directly on Goose runtime/application modules.

## Phase 5: Stabilize the provider runtime seam

Purpose: make runtime access explicit before moving provider implementations.

### Scope

Convert simple providers that currently read config/secrets directly to use `ProviderRuntime`.

Start with lightweight hosted API providers that do not touch sessions, subprocesses, downloads, or local paths.

Suggested order:

1. Avian already done as first proof.
2. NanoGPT.
3. XAI.
4. LiteLLM.
5. Tetrate or Snowflake if coupling is limited.
6. Azure only after deciding how custom auth should cross the boundary.

### Runtime methods to add only as needed

Keep the trait narrow. Add methods only when a converted provider requires them.

Likely near-term methods:

```rust
fn get_secret(&self, key: &str) -> Result<String>;
fn get_param<T: DeserializeOwned>(&self, key: &str) -> Result<T>;
fn get_optional_param<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
```

Do not add path/session/subprocess/download methods until converting a provider that needs them.

### PR shape

One provider per PR:

```text
provider-crate-extraction-phase-5a-runtime-nanogpt
provider-crate-extraction-phase-5b-runtime-xai
provider-crate-extraction-phase-5c-runtime-litellm
```

Each PR should:

1. Add only the runtime capability needed by that provider.
2. Implement it in `goose`.
3. Convert the provider to use the trait.
4. Preserve existing env/config/keyring behavior.

### Validation

For each PR:

```bash
cargo fmt --all
cargo check -p goose-providers
cargo check -p goose
cargo test -p goose <targeted tests if available>
```

## Phase 6: Move provider DTOs and accounting types to `goose-types`

Purpose: remove type-level dependency on `goose` from provider base/formatting code.

### Candidate cluster 1: usage/accounting DTOs

Move from `goose::providers::base` to `goose-types`:

- `Usage`
- `ProviderUsage`

Consider whether methods that estimate token usage should stay in `goose` or become an extension/helper. The pure DTOs should not depend on:

- conversations
- tools
- token estimators
- provider implementations
- runtime services

Suggested split:

- Move pure structs and arithmetic helpers to `goose-types`.
- Keep `ensure_tokens` or any estimator-backed behavior in `goose` as an extension trait/helper.

### Candidate cluster 2: provider metadata DTOs

Evaluate and move pure metadata/config-description DTOs:

- `ConfigKey`
- `ProviderMetadata`
- `ProviderType`
- `ModelInfo`
- catalog-facing provider metadata if it is runtime-independent

Do not move provider registry/init behavior in this phase.

### Candidate cluster 3: request/response formatting support DTOs

Move pure data used by provider format modules if independent:

- format-level request/response structs
- reasoning/thinking DTOs
- image-format or modality enums if pure

Do not move DTOs that depend on Goose conversations yet unless those conversation types have been extracted or abstracted.

### PR shape

One type cluster per PR:

```text
provider-crate-extraction-phase-6a-usage-types
provider-crate-extraction-phase-6b-metadata-types
provider-crate-extraction-phase-6c-format-types
```

### Validation

For each PR:

```bash
cargo fmt --all
cargo check -p goose-types
cargo check -p goose-providers
cargo check -p goose
cargo test -p goose-types
```

## Phase 7: Move provider-independent utility modules to `goose-providers`

Purpose: shrink provider implementation dependencies before moving providers.

### Good candidates

Move one cluster at a time:

1. HTTP status/error classification helpers.
2. Retry helpers, once they no longer depend on the full Goose provider trait.
3. Request log helpers, after path/log-directory access is abstracted or kept in `goose`.
4. Provider errors, if they fit provider implementation behavior better than shared DTO behavior.
5. API client helpers, after session/context/TLS/cert config coupling is isolated.

### Likely utility order

Recommended order:

1. `http_status`
2. `errors`
3. `retry`
4. `api_client`
5. `openai_compatible` support helpers

### Notes

`api_client` is important because most lightweight hosted providers use it. However, it currently has coupling to Goose config/session/TLS behavior. Before moving it:

- identify all direct `crate::config` reads
- identify `SESSION_ID_HEADER` usage
- identify TLS/certificate feature behavior
- introduce runtime/config inputs only for the behavior actually used

### PR shape

```text
provider-crate-extraction-phase-7a-http-status
provider-crate-extraction-phase-7b-provider-errors
provider-crate-extraction-phase-7c-retry
provider-crate-extraction-phase-7d-api-client
```

Each PR should move the utility source, dependencies, and update imports.

## Phase 8: Define the provider contract in `goose-providers`

Purpose: make provider implementations compile outside `goose`.

### Target ownership

Move provider-facing base traits/types from `goose::providers::base` toward `goose-providers`.

Likely target components:

- `Provider`
- `ProviderDef`
- `MessageStream`
- `stream_from_single_message`
- `collect_stream`
- `ProviderRetry`, if not already moved

### Main blocker

The current provider trait references Goose-owned types such as:

- conversation `Message`
- MCP `Tool`
- permissions/routing types
- extension config
- GooseMode/session/runtime behavior

Do not move the trait until those inputs are either:

1. moved as pure DTOs to `goose-types`, or
2. represented by smaller provider-facing types, or
3. passed through runtime traits/adapters.

### Suggested approach

Avoid a large all-at-once trait redesign.

1. Extract pure pieces first (`ProviderUsage`, metadata types, error types).
2. Introduce `goose-providers` versions of small provider-facing abstractions only when a moved provider needs them.
3. Keep Goose runtime orchestration in `goose`.
4. Use adapter layers temporarily if necessary, but remove them in final cleanup.

### PR shape

This phase may need several PRs:

```text
provider-crate-extraction-phase-8a-provider-usage-in-base
provider-crate-extraction-phase-8b-provider-metadata-in-base
provider-crate-extraction-phase-8c-provider-error-in-base
provider-crate-extraction-phase-8d-provider-trait-seam
```

## Phase 9: Move format modules used by simple hosted providers

Purpose: move provider request/response formatting before full providers.

### Initial candidates

OpenAI-compatible hosted providers are likely the easiest first implementation moves, so prioritize their formatting dependencies:

- `formats/openai`
- `formats/openai_responses`
- maybe shared image/tool formatting helpers
- OpenAI-compatible streaming response conversion

### Blockers to handle first

These modules likely depend on:

- Goose `Message`/`MessageContent`
- tool schemas
- provider usage/accounting types
- request log helpers
- reasoning/model config helpers

Handle blockers by moving pure DTOs or introducing minimal provider-facing abstractions.

### PR shape

```text
provider-crate-extraction-phase-9a-openai-format-types
provider-crate-extraction-phase-9b-openai-format-functions
provider-crate-extraction-phase-9c-openai-compatible-streaming
```

## Phase 10: Move the first provider implementation

Purpose: prove a provider can compile and run from `goose-providers` without depending on `goose`.

### Recommended first provider

Use Avian if the previous phases made OpenAI-compatible helpers movable. Avian is a good first candidate because it is small and delegates most behavior to OpenAI-compatible infrastructure.

If Avian is still blocked by OpenAI-compatible infrastructure, use the smallest provider whose direct dependencies have already moved.

### Avian move checklist

1. Move Avian implementation to `crates/goose-providers`.
2. Ensure Avian depends only on:
   - `goose-types`
   - `goose-providers` internals
   - external crates
   - `ProviderRuntime`
3. Move any Avian-only dependencies from `goose` to `goose-providers` if no longer needed by `goose`.
4. Update `goose` registry/init code to reference `goose-providers::avian`.
5. Preserve config keys and behavior:
   - `AVIAN_API_KEY`
   - `AVIAN_HOST`
6. Add/adjust targeted tests.

### PR shape

```text
provider-crate-extraction-phase-10-avian-provider
```

### Validation

```bash
cargo fmt --all
cargo check -p goose-providers
cargo check -p goose
cargo test -p goose-providers
cargo test -p goose <provider registry/config tests>
```

## Phase 11: Move simple hosted providers one by one

Purpose: migrate providers after the first proof is successful.

### Suggested order

Move one provider per PR:

1. Avian
2. NanoGPT
3. XAI
4. LiteLLM
5. Tetrate
6. Snowflake
7. OpenRouter
8. OpenAI
9. Anthropic
10. Google/Gemini
11. Databricks
12. Azure
13. Bedrock/SageMaker

Adjust order based on actual coupling after utilities move.

### Per-provider checklist

Each provider PR should:

1. Move provider source to `goose-providers`.
2. Move direct provider dependencies to `goose-providers/Cargo.toml`.
3. Add needed feature flags to `goose-providers`.
4. Have `goose` enable those features as needed.
5. Update registry/init/catalog references.
6. Preserve all config keys, env vars, defaults, and auth behavior.
7. Run targeted tests.

### Keep hard providers for later

Defer providers with heavier runtime coupling:

- local inference
- toolshim
- Codex/Claude Code/Gemini CLI/ACP providers
- GitHub Copilot
- OAuth-heavy providers
- providers needing download manager or local model registry

## Phase 12: Move auth helpers and OAuth flows

Purpose: move provider-owned auth behavior while keeping app/runtime storage in `goose`.

### Candidate modules

- provider OAuth/device flow helpers
- provider-specific token refresh helpers
- GCP auth helpers
- Azure auth helper if provider-owned
- Gemini/XAI/ChatGPT Codex OAuth helpers

### Runtime capabilities likely needed

Only add these when converting the first auth helper that needs them:

```rust
fn config_dir(&self) -> PathBuf;
fn state_dir(&self) -> PathBuf;
fn open_browser(&self, url: &str) -> Result<()>;
fn store_secret(&self, key: &str, value: &str) -> Result<()>;
fn delete_secret(&self, key: &str) -> Result<()>;
```

Do not make the runtime trait a general app facade. Keep methods provider-specific and motivated by converted callers.

## Phase 13: Move subprocess/code-agent providers

Purpose: migrate providers that launch external binaries.

### Candidate providers

- Codex
- Claude Code
- Gemini CLI
- ACP-style code providers
- Cursor agent
- Amp ACP
- Copilot ACP

### Runtime capabilities likely needed

Add only when needed:

```rust
fn resolve_command(&self, command: &str, search_policy: SearchPolicy) -> Result<PathBuf>;
fn configure_subprocess(&self, command: &mut Command);
fn config_value<T: DeserializeOwned>(&self, key: &str) -> Result<T>;
fn state_subdir(&self, relative: &str) -> Result<PathBuf>;
```

### Notes

This phase should come after simple hosted providers have moved. These providers are likely to expose weaknesses in the runtime abstraction and should not drive the first design.

## Phase 14: Move local inference and downloads last

Purpose: migrate the heaviest provider subsystem.

### Candidate modules

- `local_inference`
- local model registry
- Hugging Face model discovery
- download integration
- llama.cpp backend wiring
- multimodal helpers

### Expected blockers

- `download_manager`
- app data paths
- model registry persistence
- feature flags for local inference, CUDA, Vulkan, Metal
- optional native dependencies
- background progress reporting

### Strategy

1. First separate pure local-inference DTOs and model registry data from runtime file/download behavior.
2. Introduce a small download/runtime trait only for model downloads and local model paths.
3. Move local inference behind `goose-providers` feature flags.
4. Keep Goose UI/server progress orchestration in `goose`.

## Phase 15: Move provider registry/catalog/init ownership

Purpose: complete the provider surface migration.

### Move or split

Move provider-owned pieces:

- provider catalog definitions
- provider inventory where provider-specific
- provider registry metadata
- provider constructors/factories
- declarative provider parsing/formatting if provider-owned

Keep Goose-owned pieces:

- configured provider selection
- session-level provider changes
- UI/server routes
- config file persistence
- extension orchestration

### Desired final use from Goose

Goose should call APIs from `goose-providers`, for example:

```rust
goose_providers::registry::create(...)
goose_providers::catalog::providers(...)
```

rather than maintaining a long-term `goose::providers` re-export layer.

## Phase 16: Final cleanup

After all providers compile in `goose-providers` without depending on `goose`:

1. Remove remaining provider implementation modules from `crates/goose/src/providers`.
2. Remove provider-only dependencies from `crates/goose/Cargo.toml`.
3. Remove temporary adapter/re-export layers.
4. Remove temporary cargo machete ignores if any were added.
5. Audit feature flags:
   - TLS
   - AWS providers
   - local inference
   - CUDA/Vulkan/Metal
   - telemetry
   - code mode
   - keyring
   - Nostr
6. Confirm `goose-providers` does not depend on `goose`.
7. Confirm public provider APIs are consumed from `goose-providers`.

## Suggested milestone PR stack from here

A practical next stack could be:

```text
provider-crate-extraction-phase-5a-runtime-nanogpt
provider-crate-extraction-phase-5b-runtime-xai
provider-crate-extraction-phase-6a-usage-types
provider-crate-extraction-phase-6b-provider-metadata-types
provider-crate-extraction-phase-7a-http-status
provider-crate-extraction-phase-7b-provider-errors
provider-crate-extraction-phase-7c-retry
provider-crate-extraction-phase-7d-api-client
provider-crate-extraction-phase-9a-openai-format-types
provider-crate-extraction-phase-10-avian-provider
```

After Avian moves successfully, repeat one provider per PR.

## Completion checklist

The extraction is complete when:

- [ ] `goose-providers` has no dependency on `goose`.
- [ ] `goose-types` has no dependency on `goose` or `goose-providers`.
- [ ] Provider implementations live in `goose-providers`.
- [ ] Provider-facing pure DTOs live in `goose-types`.
- [ ] Goose runtime/config/session/path/download/subprocess behavior is accessed through small explicit runtime traits or remains in `goose` orchestration.
- [ ] Goose consumes provider APIs directly from `goose-providers`.
- [ ] No long-term `goose::providers` compatibility/re-export layer remains.
- [ ] Existing provider config/env behavior is preserved.
- [ ] Feature flags remain equivalent to pre-extraction behavior.
