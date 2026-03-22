from pybevy.app import Plugin

# ── Plugins ─────────────────────────────────────────────────────────────────

class McpPlugin(Plugin):
    """MCP (Model Context Protocol) server plugin for AI agent integration.

    Starts an embedded HTTP server that implements the MCP protocol,
    allowing AI agents to inspect and manipulate the running scene.

    Args:
        port: Server port (default 8420)
        host: Server bind address (default "127.0.0.1")
        screenshot: Enable screenshot capture tools (default True)
        manipulation: Enable entity/resource manipulation tools (default True)
        execute_python: Enable arbitrary Python execution (default False, opt-in for safety)
        api_discovery: Enable API stub discovery tools (default True)
    """

    def __init__(
        self,
        port: int = 8420,
        host: str = "127.0.0.1",
        screenshot: bool = True,
        manipulation: bool = True,
        execute_python: bool = False,
        api_discovery: bool = True,
    ) -> None: ...

class ApiIndex:
    """Pre-built API index from .pyi stub files.

    Provides search, type lookup, and guide access for the PyBevy API.
    Used by the MCP bridge for local API discovery without a running Bevy app.

    Args:
        pybevy_dir: Optional path to the pybevy/ directory containing .pyi stubs.
                    If not provided, auto-discovers from the current working directory.
    """

    def __init__(self, pybevy_dir: str | None = None) -> None: ...
    def search(self, query: str) -> str: ...
    def get_type_definition(self, type_name: str) -> str | None: ...
    def get_type_definition_structured(self, type_name: str) -> str | None: ...
    def get_index(self) -> str: ...
    def get_module_content(self, module_name: str) -> str | None: ...
    def get_guide_index(self) -> str: ...
    def get_guide(self, name: str) -> str | None: ...
    def get_instructions(self) -> str | None: ...
