from pybevy.app import App, Plugin

class GizmoPlugin(Plugin):
    """Gizmo plugin; requires AssetPlugin and MeshPlugin before it."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
