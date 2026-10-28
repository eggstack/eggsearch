import asyncio
import tempfile
from importlib.metadata import distribution
from pathlib import Path

from eggsearch import AsyncEggsearch, EggsearchError, tool_names


async def main():
    if len(tool_names()) != 10:
        raise AssertionError("unexpected canonical tool inventory")
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        workspace = root / "workspace"
        workspace.mkdir()
        (workspace / "README.md").write_text(
            "Distinctive local ARM wheel fixture for bounded retry policy search.",
            encoding="utf-8",
        )
        config = root / "eggsearch.toml"
        config.write_text(
            "[search]\nmode = 'off'\ndefault_max_results = 10\n"
            "max_results_cap = 50\nmax_query_chars = 512\ntimeout_ms = 8000\n"
            "default_providers = ['duckduckgo']\nsanitize_output = true\n\n"
            "[search.providers]\nduckduckgo = true\n\n"
            "[fetch]\nenabled = false\ntimeout_ms = 8000\n"
            "max_bytes = 2000000\nmax_chars_default = 12000\n"
            "max_chars_cap = 50000\nredirect_limit = 5\n"
            "allow_private_network = false\nallow_localhost = false\n"
            "sanitize_output = true\n\n"
            f"[local]\nenabled = true\nroots = ['{workspace.as_posix()}']\n",
            encoding="utf-8",
        )
        client = AsyncEggsearch(str(config))
        try:
            try:
                await client.execute("repo_search", {})
            except EggsearchError as error:
                if error.code != "locator_invalid" or not error.message:
                    raise AssertionError("malformed input returned an unexpected error") from error
            else:
                raise AssertionError("malformed repository search input was accepted")
            result = await client.repo_search(query="retry", include_local=True)
            cards = [
                card
                for group in result.get("groups", [])
                for card in group.get("results", [])
            ]
            if not any(card.get("trust") == "local_trusted" for card in cards):
                raise AssertionError("Mode-Off local search did not return trusted evidence")
        finally:
            await client.aclose()
        try:
            await client.provider_status(probe=False)
        except EggsearchError as error:
            if error.code != "client_closed":
                raise AssertionError("closed client returned an unexpected error") from error
        else:
            raise AssertionError("closed client accepted a new operation")
    print(distribution("eggsearch").version)


asyncio.run(main())
