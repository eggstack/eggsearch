# Eggsearch native Python library

The native package exposes an asyncio client for Eggsearch's ten search,
fetch, repository, security, research, diagnostic, and evidence operations.
It calls the shared Rust SDK in-process and does not start the MCP server.

This package is not published to PyPI. To build and install it locally, use
Rust 1.89, CPython 3.11–3.14, and an active virtual environment:

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install -e 'bindings/python[dev]' maturin==1.9.5
maturin develop --manifest-path bindings/python/Cargo.toml --locked
python -m pytest -q bindings/python/tests
pyright bindings/python/python/eggsearch bindings/python/examples
```

```python
import asyncio

from eggsearch import AsyncEggsearch


async def main() -> None:
    async with AsyncEggsearch() as client:
        result = await client.web_search(query="Rust async cancellation")
        print(result.get("results", []))


asyncio.run(main())
```

See [`docs/python-library.md`](../../docs/python-library.md) for configuration,
types, trust handling, and the framework-neutral search/fetch/evidence example.
