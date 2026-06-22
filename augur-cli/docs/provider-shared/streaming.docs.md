# Module: streaming

Provides Server-Sent Events (SSE) line parsing shared by the Anthropic and
OpenAI streaming providers.

The core function `drain_complete_sse_lines` is the only exported API. It
maintains a carry buffer of type `AccumulatedText` across successive HTTP
byte chunks, appending each new chunk (wrapped in `SseChunk`) via lossy
UTF-8 decoding. The function splits the accumulated text on newlines,
returns all complete non-empty lines, and retains any trailing partial line
in the carry buffer for the next invocation. This design handles the common
case where an SSE `data:` line is split across two HTTP chunk boundaries
without losing any bytes.

The `SseChunk` wrapper is a borrowed byte-slice newtype that documents the
input contract: it accepts raw HTTP body bytes and converts them to a
lossy UTF-8 string representation. Both the Anthropic and OpenAI streaming
loops call `drain_complete_sse_lines` identically, so any improvement to
the carry-buffer logic (for example, explicit line-length limits or
malformed-SSE detection) benefits both providers simultaneously. The
function returns `Vec<OutputText>` rather than streaming individual lines,
because in practice the number of complete lines per chunk is small and
the caller needs all of them to detect event/type pairs.