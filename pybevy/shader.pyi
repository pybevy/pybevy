"""PyBevy shader module for custom shader support."""

import builtins
from typing import ClassVar, Literal

from pybevy.assets import Asset
from pybevy.assets import Handle as AssetHandle
from pybevy.collections import LiveList

class ShaderImport:
    """
    Represents a shader import path.

    Shader imports can be either:
    - Asset paths: References to shader files in the asset system (e.g., "/shaders/utils"
      for the WESL asset "shaders/utils.wesl")
    - Custom: Named WESL package modules (e.g., "bevy_pbr::render::utils")
    """

    def __copy__(self) -> ShaderImport: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ShaderImport: ...


    class AssetPath(ShaderImport):
        __match_args__: ClassVar[tuple[Literal["value"]]] = ("value",)
        @property
        def value(self) -> str: ...
        def __init__(self, value: str) -> None: ...

    class Custom(ShaderImport):
        __match_args__: ClassVar[tuple[Literal["value"]]] = ("value",)
        @property
        def value(self) -> str: ...
        def __init__(self, value: str) -> None: ...

    def module_name(self) -> str:
        """Get the module name for this import."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class ValidateShader:
    """
    Describes whether or not to perform runtime checks on shaders.

    Runtime checks can be enabled for safety at the cost of speed.
    By default no runtime checks will be performed.
    """

    def __copy__(self) -> ValidateShader: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ValidateShader: ...


    Disabled: ClassVar[ValidateShader]
    """Do not perform runtime shader validation."""

    Enabled: ClassVar[ValidateShader]
    """Perform runtime shader validation."""

class ShaderDefVal:
    """
    Shader definition value for WESL conditional compilation.

    ``Bool`` definitions are ``@if`` feature flags; ``Int`` and ``UInt``
    definitions are readable as ``constants::NAME`` and also enable the flag
    of the same name.

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

    def __copy__(self) -> ShaderDefVal: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ShaderDefVal: ...


    class Bool(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]] = (
            "name",
            "value",
        )
        @property
        def name(self) -> str: ...
        @property
        def value(self) -> builtins.bool: ...
        def __init__(self, name: str, value: builtins.bool) -> None: ...

    class Int(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]] = (
            "name",
            "value",
        )
        @property
        def name(self) -> str: ...
        @property
        def value(self) -> builtins.int: ...
        def __init__(self, name: str, value: builtins.int) -> None: ...

    class UInt(ShaderDefVal):
        __match_args__: ClassVar[tuple[Literal["name"], Literal["value"]]] = (
            "name",
            "value",
        )
        @property
        def name(self) -> str: ...
        @property
        def value(self) -> builtins.int: ...
        def __init__(self, name: str, value: builtins.int) -> None: ...

    def value_as_string(self) -> str:
        """
        Get the value of this shader define as a string.

        Returns:
            Value formatted as string
        """

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
        shader_ref = ShaderRef.Default()

        # Reference by handle
        shader_handle = shaders.add(my_shader)
        shader_ref = ShaderRef.Handle(shader_handle)

        # Reference by path
        shader_ref = ShaderRef.Path("shaders/custom.wesl")
        ```
    """


    def __hash__(self) -> int: ...

    class Default(ShaderRef):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Handle(ShaderRef):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        @property
        def value(self) -> AssetHandle: ...
        def __init__(self, value: AssetHandle) -> None: ...

    class Path(ShaderRef):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        @property
        def value(self) -> str: ...
        def __init__(self, value: str) -> None: ...


class Source:
    """
    Shader source code format.

    Represents different shader source formats that can be used in Bevy.
    """

    class Wgsl(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        @property
        def value(self) -> str: ...
        def __init__(self, value: str) -> None: ...

    class Wesl(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        @property
        def value(self) -> str: ...
        def __init__(self, value: str) -> None: ...

    class SpirV(Source):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        @property
        def value(self) -> bytes: ...
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
    A shader asset containing WGSL, WESL, or SPIR-V source code.

    WESL assets (``.wesl``) are composed with ``import`` statements and
    ``@if`` conditional compilation; plain WGSL is passed through unprocessed.

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

    def __copy__(self) -> Shader: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Shader: ...

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

        Bevy composes and applies shader definitions only for WESL modules,
        where boolean definitions drive ``@if`` attributes and integer
        definitions are readable as ``constants::NAME``. A WGSL source is
        passed through unprocessed, so these definitions do not change it.

        Args:
            source: WGSL shader source code
            path: Shader path/name for debugging and imports
            shader_defs: List of shader preprocessor definitions

        Returns:
            New Shader instance with the specified definitions

        Example:
            ```python
            shader = Shader.from_wgsl_with_defs('''
                const ITERATIONS: u32 = 10u;
            ''', "custom_shader", [
                ShaderDefVal.Bool("ENABLE_FEATURE", True),
                ShaderDefVal.Int("ITERATIONS", 10),
            ])
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

    @staticmethod
    def from_wesl(source: str, path: str) -> Shader:
        """
        Create a new shader from WESL source code.

        WESL is WGSL with imports (`import a::b;`) and conditional compilation
        (`@if(FLAG)`). The path sets the module path other shaders import:
        `embedded://crate/dir/file.wesl` is `crate::dir::file`, any other path is
        imported by its asset path.

        Args:
            source: WESL shader source code
            path: Shader path, which also determines its import path

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

    @path.setter
    def path(self, value: str) -> None: ...

    @property
    def source(self) -> Source:
        """
        Get the shader source.

        Returns:
        Source instance containing shader code
        """

    @source.setter
    def source(self, value: Source) -> None: ...

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
    def imports(self) -> LiveList[ShaderImport]:
        """
        Get the live list of imports this shader depends on.

        Returns:
        Live list of ShaderImport values; edits write through to the shader
        """

    @imports.setter
    def imports(self, value: list[ShaderImport]) -> None: ...

    @property
    def shader_defs(self) -> LiveList[ShaderDefVal]:
        """
        Get the live list of shader preprocessor definitions.

        Returns:
        Live list of ShaderDefVal values; edits write through to the shader
        """

    @shader_defs.setter
    def shader_defs(self, value: list[ShaderDefVal]) -> None: ...

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
