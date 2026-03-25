"""Distance fog example.

Demonstrates:
- Distance-based fog for atmospheric effects
- Different fog falloff modes (Linear, Exponential, Atmospheric)
- Dynamic fog adjustment with keyboard controls
- Fog color and density settings
"""

from pybevy.prelude import *


@resource
class FogSettings(Resource):
    """Tracks current fog mode and density."""
    def __init__(self):
        self.mode = 0  # 0=Linear, 1=Exponential, 2=Atmospheric
        self.density = 0.05  # For exponential mode


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up scene with fog."""
    # Camera with distance fog
    commands.spawn(
        Camera3d(),
        DistanceFog(
            color=Color.srgb(0.5, 0.5, 0.6),  # Grayish fog
            falloff=FogFalloff.Linear(10.0, 50.0),  # Start at 10, full at 50
        ),
        Transform.from_xyz(0.0, 2.0, 10.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Create materials
    red_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(1.0, 0.2, 0.2),
    ))

    green_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.2, 1.0, 0.2),
    ))

    blue_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.2, 0.2, 1.0),
    ))

    # Create sphere mesh
    sphere = meshes.add(Sphere(1.0))

    # Spawn spheres at different distances
    for i in range(15):
        z = -i * 5.0  # Spread out along Z axis
        material = [red_mat, green_mat, blue_mat][i % 3]

        commands.spawn(
            Mesh3d(sphere),
            MeshMaterial3d(material),
            Transform.from_xyz(0.0, 0.0, z),
        )

    # Ground plane
    ground = materials.add(StandardMaterial(
        base_color=Color.srgb(0.3, 0.3, 0.3),
    ))

    commands.spawn(
        Mesh3d(meshes.add(Plane3d(Vec3.Y, Vec2(25.0, 25.0)))),
        MeshMaterial3d(ground),
        Transform.from_xyz(0.0, -1.0, -30.0),
    )

    # Instructions
    commands.spawn(
        Text2d("1: Linear Fog\n2: Exponential Fog\n3: Atmospheric Fog\nUp/Down: Adjust Density"),
        TextFont.from_font_size(20.0),
        TextColor.WHITE,
        Transform.from_xyz(0.0, 400.0, 0.0),
    )

    # Insert fog settings resource
    commands.insert_resource(FogSettings())


def toggle_fog_mode(
    keyboard: Res[ButtonInput],
    fog_query: Query[Mut[DistanceFog]],
    settings: ResMut[FogSettings],
) -> None:
    """Change fog mode with number keys."""
    for fog in fog_query:
        if keyboard.just_pressed(KeyCode.Digit1):
            # Linear fog
            fog.falloff = FogFalloff.Linear(10.0, 50.0)
            settings.mode = 0

        elif keyboard.just_pressed(KeyCode.Digit2):
            # Exponential fog
            fog.falloff = FogFalloff.Exponential(0.05)
            settings.mode = 1

        elif keyboard.just_pressed(KeyCode.Digit3):
            # Atmospheric fog with scattering
            fog.falloff = FogFalloff.Atmospheric(
                Vec3(0.35, 0.35, 0.4),  # Extinction (blueish)
                Vec3(0.25, 0.25, 0.3),  # Inscattering
            )
            settings.mode = 2

        # Adjust density (only for exponential mode)
        if settings.mode == 1:
            if keyboard.pressed(KeyCode.ArrowUp):
                # Increase density (more fog)
                settings.density = min(0.2, settings.density * 1.05)
                fog.falloff = FogFalloff.Exponential(settings.density)
            elif keyboard.pressed(KeyCode.ArrowDown):
                # Decrease density (less fog)
                settings.density = max(0.001, settings.density * 0.95)
                fog.falloff = FogFalloff.Exponential(settings.density)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_fog_mode)
    )


if __name__ == "__main__":
    main().run()
