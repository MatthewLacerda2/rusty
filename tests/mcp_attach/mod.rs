//! MCP attach-mode bridge end-to-end (#307).
//! Gated on `dev`. Drives `rusty::dev::mcp::run` with the `SocketClient` backend against
//! a *fake engine* — a listener speaking the #282 framed line protocol — asserting that
//! tool calls are forwarded over the socket and the framed replies are mapped to MCP
//! results. No real world is booted; the fake engine stands in for a running window.

mod bridge;
mod fake;
