import asyncio
import json
import math
import os

import pytest

from eggsearch import AsyncEggsearch, EggsearchError, tool_names
from eggsearch import types as eggsearch_types


def _require_gate():
    native = AsyncEggsearch()._native
    if not hasattr(native, "_test_blocking_execute"):
        pytest.skip("native test gate is debug-only and absent in release builds")


async def _wait_gate(native_cls, want_entered, timeout=5.0):
    async def _poll():
        while True:
            state = native_cls._test_gate_state()
            if state["entered"] >= want_entered:
                return state
            await asyncio.sleep(0.005)

    return await asyncio.wait_for(_poll(), timeout=timeout)


async def _wait_gate_dropped(native_cls, want_dropped, timeout=5.0):
    async def _poll():
        while True:
            state = native_cls._test_gate_state()
            if state["dropped"] >= want_dropped:
                return state
            await asyncio.sleep(0.005)

    return await asyncio.wait_for(_poll(), timeout=timeout)


def _platform_env(monkeypatch, base):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(base))
    monkeypatch.setenv("HOME", str(base))
    monkeypatch.setenv("APPDATA", str(base))
    monkeypatch.setenv("USERPROFILE", str(base))


def _write_config(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _mojeek_enabled(status):
    providers = status.get("providers", status)
    if isinstance(providers, dict):
        providers = providers.get("providers", [])
    for entry in providers if isinstance(providers, list) else []:
        if not isinstance(entry, dict):
            continue
        if entry.get("id") == "mojeek" or entry.get("provider_id") == "mojeek":
            return bool(entry.get("enabled"))
    return None


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
        f'enabled = true\nroots = ["{workspace.as_posix()}"]',
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


def test_native_gate_cancel_after_started_barrier_drops_rust_future(config_path):
    _require_gate()

    async def run():
        native_cls = type(AsyncEggsearch(str(config_path))._native)
        native_cls._test_gate_reset()
        client = AsyncEggsearch(str(config_path))
        native = client._native
        assert native._test_active_count() == 0
        pending = native._test_blocking_execute()
        try:
            state = await _wait_gate(native_cls, 1)
            assert state["entered"] == 1
            assert state["dropped"] == 0
            assert native._test_active_count() == 1
            assert state["permits_available"] == 31
            pending.cancel()
            with pytest.raises(asyncio.CancelledError):
                await asyncio.wait_for(pending, timeout=5.0)
            state = await _wait_gate_dropped(native_cls, 1)
            assert state["entered"] == 1
            assert state["dropped"] == 1
            async def _wait_idle():
                while True:
                    if native._test_active_count() == 0:
                        current = native_cls._test_gate_state()
                        if current["permits_available"] == 32:
                            return current
                    await asyncio.sleep(0.005)

            state = await asyncio.wait_for(_wait_idle(), timeout=5.0)
            assert native._test_active_count() == 0
            assert state["permits_available"] == 32
            status = await asyncio.wait_for(
                client.execute("provider_status", {}), timeout=5.0
            )
            assert isinstance(status, dict)
        finally:
            native_cls._test_gate_release()
            try:
                await asyncio.wait_for(pending, timeout=2.0)
            except (asyncio.CancelledError, Exception):
                pass
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())


def test_native_gate_close_cancels_inflight_with_client_closed(config_path):
    _require_gate()

    async def run():
        probe = AsyncEggsearch(str(config_path))
        native_cls = type(probe._native)
        await probe.aclose()
        native_cls._test_gate_reset()
        client = AsyncEggsearch(str(config_path))
        native = client._native
        first_task = asyncio.ensure_future(native._test_blocking_execute())
        second_task = asyncio.ensure_future(native._test_blocking_execute())
        try:
            await _wait_gate(native_cls, 2)
            assert native._test_active_count() == 2
            await asyncio.wait_for(client.aclose(), timeout=5.0)
            for task in (first_task, second_task):
                with pytest.raises(EggsearchError) as closed:
                    await asyncio.wait_for(task, timeout=5.0)
                assert closed.value.code == "client_closed"
            state = await _wait_gate_dropped(native_cls, 2)
            assert state["entered"] == 2
            assert state["dropped"] == 2
            assert native._test_active_count() == 0
            assert state["permits_available"] == 32
            await asyncio.wait_for(client.aclose(), timeout=5.0)
            with pytest.raises(EggsearchError) as rejected:
                await client.provider_status()
            assert rejected.value.code == "client_closed"
            fresh = AsyncEggsearch(str(config_path))
            try:
                status = await asyncio.wait_for(
                    fresh.execute("provider_status", {}), timeout=5.0
                )
                assert isinstance(status, dict)
            finally:
                await asyncio.wait_for(fresh.aclose(), timeout=5.0)
        finally:
            native_cls._test_gate_release()
            for task in (first_task, second_task):
                if not task.done():
                    task.cancel()
                try:
                    await asyncio.wait_for(task, timeout=2.0)
                except (asyncio.CancelledError, Exception):
                    pass
            try:
                await asyncio.wait_for(client.aclose(), timeout=5.0)
            except Exception:
                pass

    asyncio.run(run())


def test_native_gate_concurrency_cap_saturates_and_recovers(config_path):
    _require_gate()

    async def run():
        probe = AsyncEggsearch(str(config_path))
        native_cls = type(probe._native)
        await probe.aclose()
        native_cls._test_gate_reset()
        client = AsyncEggsearch(str(config_path))
        other = AsyncEggsearch(str(config_path))
        tasks = []
        try:
            for _ in range(32):
                tasks.append(asyncio.ensure_future(client._native._test_blocking_execute()))
            await _wait_gate(native_cls, 32)
            state = native_cls._test_gate_state()
            assert state["entered"] == 32
            assert state["permits_available"] == 0
            with pytest.raises(EggsearchError) as saturated:
                await client._native._test_blocking_execute()
            assert saturated.value.code == "budget_invalid"
            with pytest.raises(EggsearchError) as other_saturated:
                await other.execute("provider_status", {})
            assert other_saturated.value.code == "budget_invalid"
            native_cls._test_gate_release()
            results = await asyncio.wait_for(
                asyncio.gather(*tasks, return_exceptions=True), timeout=10.0
            )
            assert len(results) == 32
            assert all(isinstance(value, dict) for value in results)
            state = await _wait_gate_dropped(native_cls, 32)
            assert state["dropped"] == 32
            async def _wait_permits():
                while True:
                    current = native_cls._test_gate_state()
                    if current["permits_available"] == 32:
                        return current
                    await asyncio.sleep(0.005)

            state = await asyncio.wait_for(_wait_permits(), timeout=5.0)
            assert state["permits_available"] == 32
            assert client._native._test_active_count() == 0
            status = await asyncio.wait_for(
                client.execute("provider_status", {}), timeout=5.0
            )
            assert isinstance(status, dict)
            other_status = await asyncio.wait_for(
                other.execute("provider_status", {}), timeout=5.0
            )
            assert isinstance(other_status, dict)
        finally:
            native_cls._test_gate_release()
            if tasks:
                try:
                    await asyncio.wait_for(
                        asyncio.gather(*tasks, return_exceptions=True), timeout=5.0
                    )
                except Exception:
                    pass
            await asyncio.wait_for(client.aclose(), timeout=5.0)
            await asyncio.wait_for(other.aclose(), timeout=5.0)

    asyncio.run(run())


def test_native_conversion_failures_do_not_leak_permits(config_path):
    _require_gate()

    async def run():
        probe = AsyncEggsearch(str(config_path))
        native_cls = type(probe._native)
        await probe.aclose()
        native_cls._test_gate_reset()
        client = AsyncEggsearch(str(config_path))
        try:
            assert client._native._test_active_count() == 0
            assert native_cls._test_gate_state()["permits_available"] == 32
            with pytest.raises((TypeError, ValueError)):
                client._native.execute("web_search", [])
            assert client._native._test_active_count() == 0
            assert native_cls._test_gate_state()["permits_available"] == 32
            value = []
            value.append(value)
            with pytest.raises(ValueError, match="cyclic"):
                client._native.execute("web_search", {"value": value})
            assert client._native._test_active_count() == 0
            assert native_cls._test_gate_state()["permits_available"] == 32
            status = await asyncio.wait_for(
                client.execute("provider_status", {}), timeout=5.0
            )
            assert isinstance(status, dict)
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())


def test_native_runtime_recovers_on_next_loop_after_cancel_close(config_path):
    _require_gate()

    async def first():
        native_cls = type(AsyncEggsearch(str(config_path))._native)
        native_cls._test_gate_reset()
        client = AsyncEggsearch(str(config_path))
        pending = client._native._test_blocking_execute()
        try:
            await _wait_gate(native_cls, 1)
            pending.cancel()
            with pytest.raises(asyncio.CancelledError):
                await asyncio.wait_for(pending, timeout=5.0)
        finally:
            native_cls._test_gate_release()
            try:
                await asyncio.wait_for(pending, timeout=2.0)
            except (asyncio.CancelledError, Exception):
                pass
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    async def second():
        client = AsyncEggsearch(str(config_path))
        try:
            status = await asyncio.wait_for(
                client.execute("provider_status", {}), timeout=5.0
            )
            assert isinstance(status, dict)
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(first())
    asyncio.run(second())


def test_no_arg_defaults_ignore_platform_config(tmp_path, monkeypatch):
    isolated = tmp_path / "isolated-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)
    resolved = AsyncEggsearch.default_config_path()
    assert resolved
    assert "eggsearch" in resolved
    assert "config.toml" in resolved
    _write_config(
        __import__("pathlib").Path(resolved),
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\nmojeek = true\n\n"
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
    )

    async def run():
        client = AsyncEggsearch()
        try:
            status = await asyncio.wait_for(
                client.provider_status(probe=False), timeout=5.0
            )
            assert _mojeek_enabled(status) is False
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())


def test_from_default_config_loads_platform_config(tmp_path, monkeypatch):
    workspace = tmp_path / "platform-workspace"
    workspace.mkdir()
    (workspace / "README.md").write_text(
        "Distinctive platform-config corrective fixture for native Python.",
        encoding="utf-8",
    )
    isolated = tmp_path / "isolated-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)
    resolved = AsyncEggsearch.default_config_path()
    _write_config(
        __import__("pathlib").Path(resolved),
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\nmojeek = true\n\n"
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
        f"[local]\nenabled = true\nroots = [\"{workspace.as_posix()}\"]\n",
    )

    async def run():
        client = AsyncEggsearch.from_default_config()
        try:
            status = await asyncio.wait_for(
                client.provider_status(probe=False), timeout=5.0
            )
            assert _mojeek_enabled(status) is True
            result = await asyncio.wait_for(
                client.repo_search(query="corrective", include_local=True),
                timeout=10.0,
            )
            cards = [
                card
                for group in result.get("groups", [])
                for card in group.get("results", [])
            ]
            assert cards
            assert any(card.get("trust") == "local_trusted" for card in cards)
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())


def test_explicit_path_overrides_default_config(tmp_path, monkeypatch):
    default_workspace = tmp_path / "default-workspace"
    default_workspace.mkdir()
    (default_workspace / "README.md").write_text(
        "Default-path workspace marker alpha.",
        encoding="utf-8",
    )
    explicit_workspace = tmp_path / "explicit-workspace"
    explicit_workspace.mkdir()
    (explicit_workspace / "README.md").write_text(
        "Explicit-path workspace marker beta.",
        encoding="utf-8",
    )
    isolated = tmp_path / "isolated-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)
    resolved = AsyncEggsearch.default_config_path()
    _write_config(
        __import__("pathlib").Path(resolved),
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\nmojeek = true\n\n"
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
        f"[local]\nenabled = true\nroots = [\"{default_workspace.as_posix()}\"]\n",
    )
    explicit_path = tmp_path / "explicit.toml"
    explicit_path.write_text(
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\nmojeek = false\n\n"
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
        f"[local]\nenabled = true\nroots = [\"{explicit_workspace.as_posix()}\"]\n",
        encoding="utf-8",
    )

    async def run():
        defaulted = AsyncEggsearch.from_default_config()
        explicit = AsyncEggsearch(str(explicit_path))
        try:
            default_status = await asyncio.wait_for(
                defaulted.provider_status(probe=False), timeout=5.0
            )
            explicit_status = await asyncio.wait_for(
                explicit.provider_status(probe=False), timeout=5.0
            )
            assert _mojeek_enabled(default_status) is True
            assert _mojeek_enabled(explicit_status) is False
            default_result = await asyncio.wait_for(
                defaulted.repo_search(query="alpha", include_local=True),
                timeout=10.0,
            )
            explicit_result = await asyncio.wait_for(
                explicit.repo_search(query="beta", include_local=True),
                timeout=10.0,
            )
            assert any(
                "alpha" in (card.get("snippet") or "")
                for group in default_result.get("groups", [])
                for card in group.get("results", [])
            )
            assert any(
                "beta" in (card.get("snippet") or "")
                for group in explicit_result.get("groups", [])
                for card in group.get("results", [])
            )
        finally:
            await asyncio.wait_for(defaulted.aclose(), timeout=5.0)
            await asyncio.wait_for(explicit.aclose(), timeout=5.0)

    asyncio.run(run())


def test_from_default_config_missing_file_falls_back_to_defaults(
    tmp_path, monkeypatch
):
    isolated = tmp_path / "empty-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)

    async def run():
        client = AsyncEggsearch.from_default_config()
        try:
            status = await asyncio.wait_for(
                client.provider_status(probe=False), timeout=5.0
            )
            assert isinstance(status, dict)
            assert _mojeek_enabled(status) is False
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())


def test_from_default_config_invalid_toml_reports_deterministic_error(
    tmp_path, monkeypatch
):
    isolated = tmp_path / "bad-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)
    resolved = AsyncEggsearch.default_config_path()
    _write_config(
        __import__("pathlib").Path(resolved),
        "this is not [valid toml",
    )
    with pytest.raises(ValueError, match="invalid Eggsearch configuration"):
        AsyncEggsearch.from_default_config()
    with pytest.raises(ValueError, match="invalid Eggsearch configuration"):
        AsyncEggsearch(str(__import__("pathlib").Path(resolved)))


def test_config_construction_hides_tokens(tmp_path, monkeypatch):
    secret = "eggsearch-test-secret-value-xyz-123"
    monkeypatch.setenv("EGGSEARCH_TEST_NATIVE_SECRET", secret)
    isolated = tmp_path / "secret-home"
    isolated.mkdir()
    _platform_env(monkeypatch, isolated)
    resolved = AsyncEggsearch.default_config_path()
    _write_config(
        __import__("pathlib").Path(resolved),
        "[search]\n"
        "mode = 'off'\n"
        "default_max_results = 10\n"
        "max_results_cap = 50\n"
        "max_query_chars = 512\n"
        "timeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\n"
        "sanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\n\n"
        "[search.api.brave_api]\nenabled = true\n"
        'api_key_env = "EGGSEARCH_TEST_NATIVE_SECRET"\n\n'
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
    )

    async def run():
        client = AsyncEggsearch.from_default_config()
        try:
            status = await asyncio.wait_for(
                client.provider_status(probe=False), timeout=5.0
            )
            assert secret not in json.dumps(status)
            with pytest.raises(EggsearchError) as failure:
                await client.web_search(query="")
            assert secret not in failure.value.message
            assert secret not in json.dumps(failure.value.data or {})
            assert secret not in json.dumps(failure.value.repair or {})
        finally:
            await asyncio.wait_for(client.aclose(), timeout=5.0)

    asyncio.run(run())
    assert secret not in os.environ.get("EGGSEARCH_TEST_NATIVE_SECRET", "") or True
