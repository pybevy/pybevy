"""PyBevy shader module for custom shader support."""

import builtins
from typing import ClassVar, Literal

from pybevy.assets import Asset
from pybevy.assets import Handle as AssetHandle

class ShaderImport:
    """
    Represents a shader import path.

    Shader imports can be either:
    - Asset paths: References to shader files in the asset system (e.g., "shaders/utils.wgsl")
    - Custom: Named module imports (e.g., "bevy_pbr::utils")
    """

    class AssetPath(ShaderImport):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class Custom(ShaderImport):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    def module_name(self) -> str:
        """Get the module name for this import."""

    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class ValidateShader:
    """
    Describes whether or not to perform runtime checks on shaders.

    Runtime checks can be enabled for safety at the cost of speed.
    By default no runtime checks will be performed.
    """

    Disabled: ClassVar[ValidateShader]
    """Do not perform runtime shader validation."""

    Enabled: ClassVar[ValidateShader]
    """Perform runtime shader validation."""

class ShaderDefVal:
    """
    Shader preprocessor definition value.

    Used to define conditional compilation directives in shaders,
    similar to C preprocessor #define directives.

    Example:
        ```python
        # Boolean flag
        enable_shadows = ShaderDefVal.Bool("ENABLE_SHADOWS", True)

        # Integer value
        iterations = ShaderDefVal.Int("ITERATIONS", 10)

        # Unsigned integer
        array_size = ShaderDefVal.UInt("ARRAY_SIZE", 256)
        ```
    """

    class Bool(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]]
        name: str
        value: builtins.bool
        def __init__(self, name: str, value: builtins.bool) -> None: ...

    class Int(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]]
        name: str
        value: builtins.int
        def __init__(self, name: str, value: builtins.int) -> None: ...

    class UInt(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]]
        name: str
        value: builtins.int
        def __init__(self, name: str, value: builtins.int) -> None: ...

    def value_as_string(self) -> str:
        """
        Get the value of this shader define as a string.

        Returns:
            Value formatted as string
        """

    def __hash__(self) -> builtins.int: ...
    def __eq__(self, other: object) -> builtins.bool: ...
    def __ne__(self, other: object) -> builtins.bool: ...

class ShaderRef:
    """
    A reference to a shader asset.

    ShaderRef can reference a shader in three ways:
    - Default: Use the default shader for the current context
    - Handle: Reference a shader by its handle
    - Path: Reference a shader by its asset path

    Example:
        ```python
        # Use default shader
        shader_ref = ShaderRef.default()

        # Reference by handle
        shader_handle = shaders.add(my_shader)
        shader_ref = ShaderRef.Handle(shader_handle)

        # Reference by path
        shader_ref = ShaderRef.Path("shaders/custom.wgsl")
        ```
    """

    class Default(ShaderRef):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Handle(ShaderRef):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: AssetHandle
        def __init__(self, value: AssetHandle) -> None: ...

    class Path(ShaderRef):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    @staticmethod
    def default() -> ShaderRef:
        """
        Create a ShaderRef that uses the default shader.

        Returns:
            ShaderRef.Default instance

        Example:
            ```python
            shader_ref = ShaderRef.default()
            ```
        """

class Source:
    """
    Shader source code format.

    Represents different shader source formats that can be used in Bevy.
    """

    class Wgsl(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class Wesl(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class GlslVertex(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class GlslFragment(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class GlslCompute(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class SpirV(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bytes
        def __init__(self, value: bytes) -> None: ...

    def as_str(self) -> str:
        """
        Get the source code as a string.

        Returns:
            Source code string

        Raises:
            RuntimeError: If source is SPIR-V bytecode (cannot convert to string)
        """

class Shader(Asset):
    """
    A shader asset containing WGSL, GLSL, or SPIR-V source code.

    Shaders define how vertices and pixels are processed during rendering.
    Use this class to create custom rendering effects and materials.

    Example:
        ```python
        # Create a simple red shader
        shader = Shader.from_wgsl('''
            @fragment
            fn fragment() -> @location(0) vec4<f32> {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0);
            }
        ''', "red_shader")
        ```
    """

    @staticmethod
    def from_wgsl(source: str, path: str) -> Shader:
        """
        Create a new shader from WGSL source code.

        WGSL (WebGPU Shading Language) is Bevy's primary shader language.

        Args:
            source: WGSL shader source code
            path: Shader path/name for debugging and imports

        Returns:
            New Shader instance

        Example:
            ```python
            shader = Shader.from_wgsl('''
                @fragment
                fn fragment() -> @location(0) vec4<f32> {
                    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
                }
            ''', "custom_shader")
            ```
        """

    @staticmethod
    def from_wgsl_with_defs(
        source: str, path: str, shader_defs: list[ShaderDefVal]
    ) -> Shader:
        """
        Create a new shader from WGSL source code with shader definitions.

        Shader definitions allow conditional compilation of shaders based on
        runtime values, similar to C preprocessor #define directives.

        Args:
            source: WGSL shader source code
            path: Shader path/name for debugging and imports
            shader_defs: List of shader preprocessor definitions

        Returns:
            New Shader instance with the specified definitions

        Example:
            ```python
            shader = Shader.from_wgsl_with_defs('''
                #ifdef ENABLE_FEATURE
                const ITERATIONS: u32 = #{ITERATIONS};
                #endif
            ''', "custom_shader", [
                ShaderDefVal.Bool("ENABLE_FEATURE", True),
                ShaderDefVal.Int("ITERATIONS", 10),
            ])
            ```
        """

    @staticmethod
    def from_glsl(source: str, stage: str, path: str) -> Shader:
        """
        Create a new shader from GLSL source code.

        Args:
            source: GLSL shader source code
            stage: Shader stage - "vertex", "fragment", or "compute"
            path: Shader path/name for debugging

        Returns:
            New Shader instance

        Raises:
            ValueError: If stage is not "vertex", "fragment", or "compute"

        Example:
            ```python
            shader = Shader.from_glsl('''
                #version 450
                void main() {
                    gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0);
                }
            ''', "fragment", "custom_shader")
            ```
        """

    @staticmethod
    def from_spirv(source: bytes, path: str) -> Shader:
        """
        Create a new shader from SPIR-V bytecode.

        Args:
            source: SPIR-V shader bytecode
            path: Shader path/name for debugging

        Returns:
            New Shader instance
        """

    @property
    def path(self) -> str:
        """
        Get the shader path/name.

        Returns:
            Shader path string
        """

    @property
    def source(self) -> Source:
        """
        Get the shader source.

        Returns:
            Source instance containing shader code
        """

    @property
    def import_path(self) -> ShaderImport:
        """
        Get the shader's import path.

        Returns:
            ShaderImport instance representing how this shader can be imported
        """

    @import_path.setter
    def import_path(self, value: ShaderImport) -> None:
        """
        Set the shader's import path.

        Args:
            value: The ShaderImport to set
        """

    @property
    def imports(self) -> list[ShaderImport]:
        """
        Get the list of imports this shader depends on.

        Returns:
            List of ShaderImport instances representing shader dependencies
        """

    @property
    def shader_defs(self) -> list[ShaderDefVal]:
        """
        Get the shader preprocessor definitions.

        Returns:
            List of ShaderDefVal instances representing shader defines
        """

    @property
    def validate_shader(self) -> ValidateShader:
        """
        Get whether runtime shader validation is enabled.

        Returns:
            ValidateShader instance indicating validation mode
        """

    @validate_shader.setter
    def validate_shader(self, value: ValidateShader) -> None:
        """
        Set whether runtime shader validation is enabled.

        Args:
            value: ValidateShader.Enabled or ValidateShader.Disabled
        """

__all__ = [
    "Shader",
    "ShaderDefVal",
    "ShaderImport",
    "ShaderRef",
    "Source",
    "ValidateShader",
]
