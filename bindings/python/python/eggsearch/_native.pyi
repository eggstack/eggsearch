from typing import Any


class EggsearchError(Exception):
    code: str
    message: str
    repair: Any | None
    data: Any | None


class AsyncEggsearch:
    def __init__(self, config_path: str | None = ...) -> None: ...
    def execute(self, tool: str, arguments: dict[str, Any]) -> Any: ...
    def aclose(self) -> Any: ...


def tool_names() -> list[str]: ...
