# Module: request_context

Defines the command protocol between the LLM actor and the provider crates,
along with the validated request context that every provider receives.

## Command Protocol

The `LlmCommand` enum is the actor dispatch protocol. Its `Complete` variant
carries the endpoint name, message history, tool definitions, optional cache
tiers (for Anthropic prompt caching), an optional model override, and a
per-request reply channel. The `SendAutomated` variant is a lightweight path
for one-shot automated user messages that still flows through the same reply
channel mechanism. A `Shutdown` variant signals the actor loop to stop. All
variants that produce output carry their own `mpsc::Sender<StreamChunk>`
so responses are always routed back to the caller with no shared mutable
state.

## Request Context Construction

`build_request_context` transforms a `CompleteFields` bundle (route, payload,
reply sender, and optional logger) plus the application config into a
`RequestContext`. It looks up the endpoint by name from `AppConfig`, resolves
the API key for preflight validation (without storing the secret in the returned
struct -- providers read it from the environment at dispatch time), applies any
model override, and populates generation parameters (`max_tokens`,
`temperature`) from the agent config. The resulting `RequestContext` bundles
the resolved endpoint configuration, message/tool/cache payload, reply channel,
generation parameters, extra HTTP headers (populated for OpenRouter caching),
session identifier, and optional logger handle -- everything a provider needs
to build and dispatch a request without further config access.

## API Key Resolution

`resolve_api_key` validates endpoint credentials by preferring a direct
`api_key` value when set, otherwise reading the env var named by
`api_key_env`. It returns an empty `ApiKeyValue` for unauthenticated
endpoints (neither field set) and an `Err(EnvVarName)` when the named
environment variable is absent. This function is used both for preflight
validation in `build_request_context` and at actual dispatch time by the
Anthropic and OpenAI providers.