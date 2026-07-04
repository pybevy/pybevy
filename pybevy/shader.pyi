"""PyBevy shader module for custom shader support."""

import builtins

from pybevy.assets import Asset, Handle

class ShaderImport:
    """
    Represents a shader import path.

    Shader imports can be either:
    - Asset paths: References to shader files in the asset system (e.g., "shaders/utils.wgsl")
    - Custom: Named module imports (e.g., "bevy_pbr::utils")
    """

    @staticmethod
    def asset_path(path: str) -> ShaderImport:
        """Create an import from an asset path."""

    @staticmethod
    def custom(name: str) -> ShaderImport:
        """Create a custom named import."""

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

    @staticmethod
    def disabled() -> ValidateShader:
        """
        No runtime checks for soundness (e.g. bound checking) are performed.

        This is suitable for trusted shaders, written by your program or dependencies you trust.
        """

    @staticmethod
    def enabled() -> ValidateShader:
        """
        Enable's runtime checks for soundness (e.g. bound checking).

        While this can have a meaningful impact on performance,
        this setting should always be enabled when loading untrusted shaders.
        """

class ShaderDefVal:
    """
    Shader preprocessor definition value.

    Used to define conditional compilation directives in shaders,
    similar to C preprocessor #define directives.

    Example:
        ```python
        # Boolean flag
        enable_shadows = ShaderDefVal.bool("ENABLE_SHADOWS", True)

        # Integer value
        iterations = ShaderDefVal.int("ITERATIONS", 10)

        # Unsigned integer
        array_size = ShaderDefVal.uint("ARRAY_SIZE", 256)
        ```
    """

    @staticmethod
    def bool(name: str, value: builtins.bool) -> ShaderDefVal:
        """
        Create a boolean shader define.

        Args:
            name: Define name
            value: Boolean value

        Returns:
            ShaderDefVal instance

        Example:
            ```python
            # Enable a feature flag
            define = ShaderDefVal.bool("ENABLE_SHADOWS", True)
            ```
        """

    @staticmethod
    def int(name: str, value: builtins.int) -> ShaderDefVal:
        """
        Create an integer shader define.

        Args:
            name: Define name
            value: Integer value

        Returns:
            ShaderDefVal instance

        Example:
            ```python
            # Set iteration count
            define = ShaderDefVal.int("ITERATIONS", 10)
            ```
        """

    @staticmethod
    def uint(name: str, value: builtins.int) -> ShaderDefVal:
        """
        Create an unsigned integer shader define.

        Args:
            name: Define name
            value: Unsigned integer value (must be non-negative)

        Returns:
            ShaderDefVal instance

        Example:
            ```python
            # Set array size
            define = ShaderDefVal.uint("ARRAY_SIZE", 256)
            ```
        """

    @property
    def name(self) -> str:
        """
        Get the name of this shader define.

        Returns:
            Define name as string
        """

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
        shader_ref = ShaderRef.from_handle(shader_handle)

        # Reference by path
        shader_ref = ShaderRef.from_path("shaders/custom.wgsl")
        ```
    """

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

    @staticmethod
    def from_handle(handle: Handle) -> ShaderRef:
        """
        Create a ShaderRef from a shader handle.

        Args:
            handle: Handle to a Shader asset

        Returns:
            ShaderRef.Handle instance

        Example:
            ```python
            shader_handle = shaders.add(my_shader)
            shader_ref = ShaderRef.from_handle(shader_handle)
            ```
        """

    @staticmethod
    def from_path(path: str) -> ShaderRef:
        """
        Create a ShaderRef from an asset path.

        Args:
            path: Path to shader asset file

        Returns:
            ShaderRef.Path instance

        Example:
            ```python
            shader_ref = ShaderRef.from_path("shaders/custom.wgsl")
            ```
        """

class Source:
    """
    Shader source code format.

    Represents different shader source formats that can be used in Bevy.
    """

    @staticmethod
    def wgsl(source: str) -> Source:
        """
        Create WGSL (WebGPU Shading Language) shader source.

        Args:
            source: WGSL shader source code

        Returns:
            Source instance with WGSL code
        """

    @staticmethod
    def wesl(source: str) -> Source:
        """
        Create WESL (WebGPU Extended Shading Language) shader source.

        WESL is an extension of WGSL with additional features.

        Args:
            source: WESL shader source code

        Returns:
            Source instance with WESL code
        """

    @staticmethod
    def glsl_vertex(source: str) -> Source:
        """
        Create GLSL vertex shader source.

        Args:
            source: GLSL vertex shader source code

        Returns:
            Source instance with GLSL vertex shader
        """

    @staticmethod
    def glsl_fragment(source: str) -> Source:
        """
        Create GLSL fragment shader source.

        Args:
            source: GLSL fragment shader source code

        Returns:
            Source instance with GLSL fragment shader
        """

    @staticmethod
    def glsl_compute(source: str) -> Source:
        """
        Create GLSL compute shader source.

        Args:
            source: GLSL compute shader source code

        Returns:
            Source instance with GLSL compute shader
        """

    @staticmethod
    def spirv(bytecode: bytes) -> Source:
        """
        Create SPIR-V shader bytecode.

        Args:
            bytecode: SPIR-V shader bytecode

        Returns:
            Source instance with SPIR-V bytecode
        """

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
                ShaderDefVal.bool("ENABLE_FEATURE", True),
                ShaderDefVal.int("ITERATIONS", 10),
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
