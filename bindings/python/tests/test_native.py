import asyncio
import math

import pytest

from eggsearch import AsyncEggsearch, EggsearchError, tool_names
from eggsearch import types as eggsearch_types


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

REQUEST_TYPES = {
    "web_search": eggsearch_types.WebSearchRequest,
    "web_fetch": eggsearch_types.WebFetchRequest,
    "batch_fetch": eggsearch_types.BatchFetchRequest,
    "provider_status": eggsearch_types.ProviderStatusRequest,
    "repo_search": eggsearch_types.RepoSearchRequest,
    "repo_fetch": eggsearch_types.RepoFetchRequest,
    "repo_map": eggsearch_types.RepoMapRequest,
    "security_search": eggsearch_types.SecuritySearchRequest,
    "research_search": eggsearch_types.ResearchSearchRequest,
    "build_evidence_bundle": eggsearch_types.EvidenceBundleRequest,
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


def test_typed_requests_cover_canonical_rust_input_schemas():
    client = AsyncEggsearch()
    contracts = client.tool_contracts()
    schemas = {
        contract["name"]: contract["input_schema"]
        for contract in contracts["tools"]
    }

    assert len(contracts["fingerprint"]) > 0
    assert set(schemas) == EXPECTED_TOOLS
    for name, request_type in REQUEST_TYPES.items():
        properties = schemas[name].get("properties", {})
        assert set(properties).issubset(request_type.__annotations__)


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
                await client.provider_status()
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
        client._native.execute("web_search", arguments)

    async def close():
        await client.aclose()

    asyncio.run(close())


def test_native_boundary_rejects_cycles(config_path):
    value = []
    value.append(value)
    client = AsyncEggsearch(str(config_path))
    with pytest.raises(ValueError, match="cyclic"):
        client._native.execute("web_search", {"value": value})

    async def close():
        await client.aclose()

    asyncio.run(close())


def test_native_futures_cancel_and_client_close_is_bounded(config_path):
    async def run():
        client = AsyncEggsearch(str(config_path))
        pending = client._native.execute("provider_status", {})
        pending.cancel()
        with pytest.raises(asyncio.CancelledError):
            await pending
        await client.aclose()
        with pytest.raises(EggsearchError):
            await client.provider_status()

    asyncio.run(run())


def test_native_runtime_supports_sequential_python_event_loops(config_path):
    async def run_once():
        client = AsyncEggsearch(str(config_path))
        status = await client.execute("provider_status", {})
        assert isinstance(status, dict)
        await client.aclose()

    asyncio.run(run_once())
    asyncio.run(run_once())


def test_named_methods_delegate_without_rewriting_arguments():
    methods = {
        "web_search": {
            "query": "parity",
            "providers": [],
            "max_results": 0,
            "safe_search": None,
        },
        "web_fetch": {"url": "https://example.invalid", "include_links": False},
        "batch_fetch": {"items": [], "continue_on_error": False},
        "provider_status": {"probe": False, "recipe_detail": "none"},
        "repo_search": {"query": "local", "include_local": True},
        "repo_fetch": {"owner": "acme", "repo": "widget", "path": "README.md"},
        "repo_map": {"owner": "acme", "repo": "widget", "include_files": False},
        "security_search": {"cve_id": "CVE-2024-1234", "providers": []},
        "research_search": {"query": "parity", "constraints": []},
        "build_evidence_bundle": {"sources": [], "fetches": []},
    }

    async def run():
        client = AsyncEggsearch()
        calls = []

        async def execute(tool, arguments):
            calls.append((tool, arguments))
            return {"tool": tool, "arguments": arguments}

        client.execute = execute
        for name, arguments in methods.items():
            assert await getattr(client, name)(**arguments) == {
                "tool": name,
                "arguments": arguments,
            }
        assert calls == list(methods.items())

    asyncio.run(run())


def test_async_context_manager_closes_native_client(config_path):
    async def run():
        async with AsyncEggsearch(str(config_path)) as client:
            assert isinstance(await client.provider_status(), dict)
        with pytest.raises(EggsearchError) as closed:
            await client.provider_status()
        assert closed.value.code == "client_closed"

    asyncio.run(run())


def test_public_error_exposes_structured_fields(config_path):
    async def run():
        async with AsyncEggsearch(str(config_path)) as client:
            with pytest.raises(EggsearchError) as failure:
                await client.web_search(query="")
            assert failure.value.code
            assert failure.value.message
            assert failure.value.repair is None or isinstance(failure.value.repair, dict)
            assert failure.value.data is None or isinstance(failure.value.data, dict)

    asyncio.run(run())


def test_mode_off_repo_search_returns_local_trusted_results(config_path, tmp_path):
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "README.md").write_text(
        "Distinctive local retry-policy fixture for native Python search.",
        encoding="utf-8",
    )
    config = config_path.read_text(encoding="utf-8")
    config = config.replace(
        "enabled = false\nroots = []",
        f'enabled = true\nroots = ["{workspace}"]',
    )
    config_path.write_text(config, encoding="utf-8")

    async def run():
        async with AsyncEggsearch(str(config_path)) as client:
            result = await client.repo_search(
                query="retry",
                include_local=True,
            )
            cards = [
                card
                for group in result.get("groups", [])
                for card in group.get("results", [])
            ]
            assert cards
            selected = next(
                card for card in cards if card.get("trust") == "local_trusted"
            )
            bundle = await client.build_evidence_bundle(
                goal="Review local retry policy",
                sources=[
                    {
                        "id": selected.get("id"),
                        "url": selected.get("url"),
                        "title": selected.get("title"),
                        "snippet": selected.get("snippet"),
                        "providers": [selected.get("provider_id", "local_workspace")],
                        "trust": selected.get("trust"),
                        "trust_markers": selected.get("trust_markers", {}),
                    }
                ],
            )
            assert bundle.get("bundle_id")
            assert bundle.get("sources")
            assert bundle["sources"][0].get("trust") == "local_trusted"

    asyncio.run(run())
