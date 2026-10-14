import asyncio
import math

import pytest

from eggsearch import AsyncEggsearch, EggsearchError, tool_names


EXPECTED_TOOLS = {
    "web_search",
    "web_fetch",
    "batch_fetch",
    "provider_status",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "security_search",
    "research_search",
    "build_evidence_bundle",
}


@pytest.fixture
def config_path(tmp_path):
    path = tmp_path / "eggsearch.toml"
    path.write_text(
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\n\n"
        "[fetch]\n"
        "enabled = false\n"
        "timeout_ms = 8000\n"
        "max_bytes = 2000000\n"
        "max_chars_default = 12000\n"
        "max_chars_cap = 50000\n"
        "redirect_limit = 5\n"
        "allow_private_network = false\n"
        "allow_localhost = false\n"
        "sanitize_output = true\n\n"
        "[local]\nenabled = false\nroots = []\n",
        encoding="utf-8",
    )
    return path


def test_native_module_import_and_tool_inventory():
    names = tool_names()
    assert len(names) == 10
    assert set(names) == EXPECTED_TOOLS
    assert len(set(names)) == len(names)


def test_native_client_executes_all_operations_without_an_mcp_process(config_path):
    arguments = {
        "web_search": {"query": "native parity"},
        "web_fetch": {"url": "file:///etc/passwd"},
        "batch_fetch": {"items": []},
        "provider_status": {},
        "repo_search": {"query": "native parity"},
        "repo_fetch": {"owner": "invalid owner", "repo": "bad", "path": "README.md"},
        "repo_map": {"owner": "invalid owner", "repo": "bad"},
        "security_search": {"query": "CVE-2024-1234"},
        "research_search": {"query": "native parity"},
        "build_evidence_bundle": {"sources": [{}]},
    }

    async def run():
        client = AsyncEggsearch(str(config_path))
        outcomes = await asyncio.gather(
            *(client.execute(name, value) for name, value in arguments.items()),
            return_exceptions=True,
        )
        assert len(outcomes) == 10
        assert all(isinstance(value, (dict, EggsearchError)) for value in outcomes)
        assert isinstance(outcomes[3], dict)
        assert isinstance(outcomes[9], dict)
        assert isinstance(outcomes[0], EggsearchError)
        assert outcomes[0].code
        assert outcomes[0].message
        await client.aclose()
        await client.aclose()
        with pytest.raises(EggsearchError) as closed:
            client.execute("provider_status", {})
        assert closed.value.code == "client_closed"

    asyncio.run(run())


@pytest.mark.parametrize(
    "arguments",
    [
        [],
        {1: "not a string key"},
        {"value": object()},
        {"value": math.inf},
        {"value": 10**100},
        {"value": "x" * 1_100_000},
        {"value": "\ud800"},
    ],
)
def test_native_boundary_rejects_invalid_or_oversized_inputs(config_path, arguments):
    client = AsyncEggsearch(str(config_path))
    with pytest.raises((TypeError, ValueError)):
        client.execute("web_search", arguments)

    async def close():
        await client.aclose()

    asyncio.run(close())


def test_native_boundary_rejects_cycles(config_path):
    value = []
    value.append(value)
    client = AsyncEggsearch(str(config_path))
    with pytest.raises(ValueError, match="cyclic"):
        client.execute("web_search", {"value": value})

    async def close():
        await client.aclose()

    asyncio.run(close())


def test_native_futures_cancel_and_client_close_is_bounded(config_path):
    async def run():
        client = AsyncEggsearch(str(config_path))
        pending = client.execute("provider_status", {})
        pending.cancel()
        with pytest.raises(asyncio.CancelledError):
            await pending
        await client.aclose()
        with pytest.raises(EggsearchError):
            client.execute("provider_status", {})

    asyncio.run(run())


def test_native_runtime_supports_sequential_python_event_loops(config_path):
    async def run_once():
        client = AsyncEggsearch(str(config_path))
        status = await client.execute("provider_status", {})
        assert isinstance(status, dict)
        await client.aclose()

    asyncio.run(run_once())
    asyncio.run(run_once())
