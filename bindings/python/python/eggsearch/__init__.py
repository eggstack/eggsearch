from __future__ import annotations

from pathlib import Path
from typing import Any, Unpack, cast

from ._native import AsyncEggsearch as _NativeAsyncEggsearch
from ._native import EggsearchError, tool_names
from .types import (
    BatchFetchRequest,
    BatchFetchResponse,
    EvidenceBundleRequest,
    EvidenceBundleResponse,
    ProviderStatusRequest,
    ProviderStatusResponse,
    RepoFetchRequest,
    RepoFetchResponse,
    RepoMapRequest,
    RepoMapResponse,
    RepoSearchRequest,
    RepoSearchResponse,
    ResearchSearchRequest,
    ResearchSearchResponse,
    SecuritySearchRequest,
    SecuritySearchResponse,
    ToolContractSet,
    WebFetchRequest,
    WebFetchResponse,
    WebSearchRequest,
    WebSearchResponse,
)


class AsyncEggsearch:
    """Async in-process client for Eggsearch's canonical tool operations."""

    def __init__(self, config_path: str | Path | None = None) -> None:
        self._native = _NativeAsyncEggsearch(
            None if config_path is None else str(config_path)
        )

    @classmethod
    def from_default_config(cls) -> AsyncEggsearch:
        instance = cls.__new__(cls)
        instance._native = _NativeAsyncEggsearch.from_default_config()
        return instance

    @staticmethod
    def default_config_path() -> str:
        return str(_NativeAsyncEggsearch.default_config_path())

    async def __aenter__(self) -> AsyncEggsearch:
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        await self._native.aclose()

    async def execute(self, tool: str, arguments: dict[str, Any]) -> dict[str, Any]:
        return await self._native.execute(tool, arguments)

    def tool_contracts(self) -> ToolContractSet:
        """Return the fingerprint and canonical Rust input/output schemas."""
        return cast(ToolContractSet, self._native.tool_contracts())

    async def web_search(
        self, **kwargs: Unpack[WebSearchRequest]
    ) -> WebSearchResponse:
        return cast(
            WebSearchResponse,
            await self.execute("web_search", cast(dict[str, Any], kwargs)),
        )

    async def web_fetch(self, **kwargs: Unpack[WebFetchRequest]) -> WebFetchResponse:
        return cast(
            WebFetchResponse,
            await self.execute("web_fetch", cast(dict[str, Any], kwargs)),
        )

    async def batch_fetch(
        self, **kwargs: Unpack[BatchFetchRequest]
    ) -> BatchFetchResponse:
        return cast(
            BatchFetchResponse,
            await self.execute("batch_fetch", cast(dict[str, Any], kwargs)),
        )

    async def provider_status(
        self, **kwargs: Unpack[ProviderStatusRequest]
    ) -> ProviderStatusResponse:
        return cast(
            ProviderStatusResponse,
            await self.execute("provider_status", cast(dict[str, Any], kwargs)),
        )

    async def repo_search(
        self, **kwargs: Unpack[RepoSearchRequest]
    ) -> RepoSearchResponse:
        return cast(
            RepoSearchResponse,
            await self.execute("repo_search", cast(dict[str, Any], kwargs)),
        )

    async def repo_fetch(
        self, **kwargs: Unpack[RepoFetchRequest]
    ) -> RepoFetchResponse:
        return cast(
            RepoFetchResponse,
            await self.execute("repo_fetch", cast(dict[str, Any], kwargs)),
        )

    async def repo_map(self, **kwargs: Unpack[RepoMapRequest]) -> RepoMapResponse:
        return cast(
            RepoMapResponse,
            await self.execute("repo_map", cast(dict[str, Any], kwargs)),
        )

    async def security_search(
        self, **kwargs: Unpack[SecuritySearchRequest]
    ) -> SecuritySearchResponse:
        return cast(
            SecuritySearchResponse,
            await self.execute("security_search", cast(dict[str, Any], kwargs)),
        )

    async def research_search(
        self, **kwargs: Unpack[ResearchSearchRequest]
    ) -> ResearchSearchResponse:
        return cast(
            ResearchSearchResponse,
            await self.execute("research_search", cast(dict[str, Any], kwargs)),
        )

    async def build_evidence_bundle(
        self, **kwargs: Unpack[EvidenceBundleRequest]
    ) -> EvidenceBundleResponse:
        return cast(
            EvidenceBundleResponse,
            await self.execute(
                "build_evidence_bundle",
                cast(dict[str, Any], kwargs),
            ),
        )


__all__ = [
    "AsyncEggsearch",
    "EggsearchError",
    "tool_names",
]
