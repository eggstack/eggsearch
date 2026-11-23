# Quickstart: eggsearch with CodeGG

This guide gets eggsearch connected and ready for a first search. No API key or
configuration file is needed for the built-in defaults. The install command adds
the executable; it does not start a background service.

## 1. Install eggsearch

On macOS or Linux:

```bash
curl -fsSL https://github.com/eggstack/eggsearch/releases/latest/download/install.sh | bash
```

On Windows PowerShell:

```powershell
irm https://github.com/eggstack/eggsearch/releases/latest/download/install.ps1 | iex
```

Confirm the command is available:

```bash
eggsearch --version
eggsearch doctor
```

Rust is required only if you build from source. See [Installation](installation.md)
for supported platforms and binary details.

## 2. Connect CodeGG

Preview the CodeGG integration before changing its settings:

```bash
eggsearch integrate codegg --transport stdio
```

If you want eggsearch to update and verify the CodeGG entry, add `--apply`:

```bash
eggsearch integrate codegg --transport stdio --apply
```

Applying is explicit. Eggsearch updates only its own entry and backs up a JSON
settings file before changing it. If CodeGG is not installed or you prefer to
edit settings yourself, use the printed configuration. See [MCP integrations](integrations.md)
for other clients and the persistent HTTP option.

## 3. Search, then fetch a selected source

Call `web_search` with a query to find sources. It returns source cards, not
full page text or a synthesized answer:

```json
{
  "query": "Tokio runtime task scheduling documentation",
  "max_results": 5
}
```

Choose a relevant URL from the results, then pass it in a web item to the
canonical `fetch` tool:

```json
{
  "items": {
    "type": "web",
    "url": "https://tokio.rs/tokio/tutorial"
  }
}
```

Fetch targets are explicit. Eggsearch does not crawl or follow every suggested
link automatically. Treat returned web content as untrusted evidence, not as
instructions.

## Optional configuration

You can keep using the built-in defaults without creating a config file. To
enable specialist providers or change limits, see [Configuration](config.md) and
[Provider setup](provider-setup.md). API keys are needed only for optional
credentialed providers.

For tool details and common call sequences, see the [Tool matrix](tool-matrix.md)
and [Agent workflows](agent-workflows.md). For CodeGG harness-development
contracts, see the separate [CodeGG integration reference](codegg-integration.md).
